//! Data Manipulation Language (DML) primitives.
//!
//! This module contains pure functions for computing the resulting state of a relation
//! after applying `UPDATE` or `DELETE` operations. These functions are intentionally decoupled
//! from the `Database` struct and storage engine to facilitate testing and optimize
//! tuple evaluation loops.
use crate::error::DatabaseError;
use crate::values::{Relation, Tuple};

pub(crate) fn compute_relation_after_delete<F>(
    current_relation: Relation,
    predicate: F,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
{
    let initial_cardinality = current_relation.cardinality();

    // Optimization: Use `restrict_into` to mutate the relation in-place.
    // By keeping the tuples where `!predicate` is true, we delete the ones
    // where `predicate` is true, without allocating a new `HashSet` or
    // cloning the surviving tuples.
    let new_relation = current_relation.restrict_into(|tuple| !predicate(tuple));

    let delete_count = initial_cardinality - new_relation.cardinality();

    Ok((new_relation, delete_count))
}

pub(crate) fn compute_relation_after_update<F, U>(
    current_relation: Relation,
    predicate: F,
    updater: U,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
    U: Fn(&Tuple) -> Tuple,
{
    let relation_type = current_relation.relation_type().clone();
    let expected_type = relation_type.tuple_type(); // Borrow instead of cloning Arc<TupleType>
    let initial_cardinality = current_relation.cardinality();

    let (body, update_count) = current_relation.into_iter().try_fold(
        (
            std::collections::HashSet::with_capacity(initial_cardinality),
            0,
        ),
        |(mut acc, count), tuple| {
            if !predicate(&tuple) {
                acc.insert(tuple);
                return Ok((acc, count));
            }

            let updated_tuple = updater(&tuple);

            if !updated_tuple.conforms_to(expected_type) {
                return Err(DatabaseError::TupleMismatch);
            }

            acc.insert(updated_tuple);
            Ok((acc, count + 1))
        },
    )?;

    // Optimization: The updated tuples are already validated via `conforms_to`
    // inside the `try_fold` loop. By accumulating directly into a `HashSet`,
    // we eliminate an intermediate `Vec` allocation entirely.
    let new_relation = Relation::from_body_unchecked(relation_type, body);

    Ok((new_relation, update_count))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn test_relation() -> Relation {
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        );
        let mut rel = Relation::new(rel_type);
        rel.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
        rel.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
        rel
    }

    #[test]
    fn should_delete_matching_tuples_and_return_count() {
        let rel = test_relation();
        let (new_rel, count) =
            compute_relation_after_delete(rel, |t| t.get_typed::<i64>("id").unwrap() == 1).unwrap();

        assert_eq!(count, 1, "Should delete exactly one tuple");
        assert_eq!(new_rel.cardinality(), 1, "Should have one tuple remaining");
        assert_eq!(
            new_rel
                .tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("id")
                .unwrap(),
            2,
            "Remaining tuple should be Bob"
        );
    }

    #[test]
    fn should_update_matching_tuples_and_return_count() {
        let rel = test_relation();
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_t| tuple! { id: 1i64, name: "Alicia" },
        )
        .unwrap();

        assert_eq!(count, 1, "Should update exactly one tuple");
        assert_eq!(new_rel.cardinality(), 2, "Should still have two tuples");

        let mut found_alicia = false;
        for t in new_rel.tuples() {
            if t.get_typed::<String>("name").unwrap() == "Alicia" {
                found_alicia = true;
                break;
            }
        }
        assert!(found_alicia, "Updated tuple should be present");
    }

    #[test]
    fn should_return_error_when_updated_tuple_mismatches_schema() {
        let rel = test_relation();
        let err = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            // Return a tuple with a different schema (missing 'name', adding 'age')
            |_t| tuple! { id: 1i64, age: 30i64 },
        )
        .unwrap_err();

        assert!(
            matches!(err, DatabaseError::TupleMismatch),
            "Should return TupleMismatch when updated tuple doesn't conform to relation type"
        );
    }

    #[test]
    fn should_return_zero_when_update_has_no_matches() {
        let rel = test_relation();
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 99,
            |t| t.clone(),
        )
        .unwrap();

        assert_eq!(count, 0, "Should update zero tuples");
        assert_eq!(new_rel.cardinality(), 2, "Relation should be unchanged");
    }
}
