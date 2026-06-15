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
    use crate::values::ScalarValue;

    fn setup_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut rel = Relation::new(rel_type);
        rel.insert(tuple! { id: 1i64, val: 10i64 }).unwrap();
        rel.insert(tuple! { id: 2i64, val: 20i64 }).unwrap();
        rel.insert(tuple! { id: 3i64, val: 30i64 }).unwrap();
        rel
    }

    #[test]
    fn should_delete_matching_tuples() {
        let rel = setup_relation();
        let (new_rel, count) =
            compute_relation_after_delete(rel, |t| t.get_typed::<i64>("id").unwrap() == 2).unwrap();

        assert_eq!(count, 1);
        assert_eq!(new_rel.cardinality(), 2);
    }

    #[test]
    fn should_delete_all_tuples() {
        let rel = setup_relation();
        let (new_rel, count) = compute_relation_after_delete(rel, |_| true).unwrap();

        assert_eq!(count, 3);
        assert_eq!(new_rel.cardinality(), 0);
    }

    #[test]
    fn should_delete_no_tuples_if_no_match() {
        let rel = setup_relation();
        let (new_rel, count) =
            compute_relation_after_delete(rel, |t| t.get_typed::<i64>("id").unwrap() == 99)
                .unwrap();

        assert_eq!(count, 0);
        assert_eq!(new_rel.cardinality(), 3);
    }

    #[test]
    fn should_update_matching_tuples() {
        let rel = setup_relation();
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |t| {
                let mut new_t = t.clone();
                new_t.set("val".to_string(), ScalarValue::Int(200)).unwrap();
                new_t
            },
        )
        .unwrap();

        assert_eq!(count, 1);
        assert_eq!(new_rel.cardinality(), 3);
        let updated_tuple = new_rel
            .into_iter()
            .find(|t| t.get_typed::<i64>("id").unwrap() == 2)
            .unwrap();
        assert_eq!(updated_tuple.get_typed::<i64>("val").unwrap(), 200);
    }

    #[test]
    fn should_update_no_tuples_if_no_match() {
        let rel = setup_relation();
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 99,
            |t| t.clone(),
        )
        .unwrap();

        assert_eq!(count, 0);
        assert_eq!(new_rel.cardinality(), 3);
    }

    #[test]
    fn should_merge_identical_tuples_on_update() {
        let rel = setup_relation();
        // Update tuple 2 to look exactly like tuple 1
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |_| tuple! { id: 1i64, val: 10i64 },
        )
        .unwrap();

        // 1 tuple was evaluated for update
        assert_eq!(count, 1);
        // The identical tuples merge, reducing total cardinality from 3 to 2
        assert_eq!(new_rel.cardinality(), 2);
    }

    #[test]
    fn should_return_error_on_type_mismatch() {
        let rel = setup_relation();
        let result = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |_| {
                // Return a tuple with a different schema
                tuple! { wrong_col: 1i64 }
            },
        );

        assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
    }
}
