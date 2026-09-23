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

    #[test]
    fn test_compute_relation_after_delete() -> Result<(), Box<dyn std::error::Error>> {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let mut rel = Relation::new(rel_type);
        rel.insert(tuple! { id: 1i64 })?;
        rel.insert(tuple! { id: 2i64 })?;

        // Delete id == 1
        let (new_rel, count) = compute_relation_after_delete(rel, |t| {
            t.get_typed::<i64>("id").unwrap_or_default() == 1
        })?;

        assert_eq!(count, 1);
        assert_eq!(new_rel.cardinality(), 1);
        assert_eq!(
            new_rel
                .tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("id")
                .unwrap(),
            2
        );
        Ok(())
    }

    #[test]
    fn test_compute_relation_after_update() -> Result<(), Box<dyn std::error::Error>> {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let mut rel = Relation::new(rel_type.clone());
        rel.insert(tuple! { id: 1i64 })?;
        rel.insert(tuple! { id: 2i64 })?;

        // Update id == 1 to id == 10
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap_or_default() == 1,
            |_t| tuple! { id: 10i64 },
        )?;

        assert_eq!(count, 1);
        assert_eq!(new_rel.cardinality(), 2);

        let mut ids: Vec<i64> = new_rel
            .tuples()
            .map(|t| t.get_typed::<i64>("id").unwrap_or_default())
            .collect();
        ids.sort();
        assert_eq!(ids, vec![2, 10]);
        Ok(())
    }

    #[test]
    fn test_compute_relation_after_update_no_match() -> Result<(), Box<dyn std::error::Error>> {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let mut rel = Relation::new(rel_type.clone());
        rel.insert(tuple! { id: 1i64 })?;
        rel.insert(tuple! { id: 2i64 })?;

        // Update id == 3 to id == 10 (no match)
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap_or_default() == 3,
            |_t| tuple! { id: 10i64 },
        )?;

        assert_eq!(count, 0);
        assert_eq!(new_rel.cardinality(), 2);
        Ok(())
    }

    #[test]
    fn test_compute_relation_after_update_tuple_mismatch() -> Result<(), Box<dyn std::error::Error>>
    {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let mut rel = Relation::new(rel_type.clone());
        rel.insert(tuple! { id: 1i64 })?;

        // Update id == 1 to wrong attribute
        let result = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap_or_default() == 1,
            |_t| tuple! { wrong_id: 10i64 },
        );

        assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
        Ok(())
    }
}
