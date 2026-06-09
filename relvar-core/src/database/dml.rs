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
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::{Relation, ScalarValue, Tuple};
    use std::collections::HashSet;

    fn test_relation() -> Relation {
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("val", ScalarType::Int),
        );
        let mut body = HashSet::new();
        let t1 = Tuple::new(
            rel_type.tuple_type().clone(),
            vec![
                ("id".to_string(), ScalarValue::Int(1)),
                ("val".to_string(), ScalarValue::Int(10)),
            ],
        )
        .unwrap();
        body.insert(t1);

        let t2 = Tuple::new(
            rel_type.tuple_type().clone(),
            vec![
                ("id".to_string(), ScalarValue::Int(2)),
                ("val".to_string(), ScalarValue::Int(20)),
            ],
        )
        .unwrap();
        body.insert(t2);

        let t3 = Tuple::new(
            rel_type.tuple_type().clone(),
            vec![
                ("id".to_string(), ScalarValue::Int(3)),
                ("val".to_string(), ScalarValue::Int(30)),
            ],
        )
        .unwrap();
        body.insert(t3);

        Relation::from_body_unchecked(rel_type, body)
    }

    #[test]
    fn test_compute_relation_after_delete() {
        let rel = test_relation();
        let (new_rel, count) = compute_relation_after_delete(rel, |t| {
            t.get_typed::<i64>("id").unwrap() == 1 || t.get_typed::<i64>("id").unwrap() == 2
        })
        .unwrap();

        assert_eq!(count, 2);
        assert_eq!(new_rel.cardinality(), 1);

        let remaining_tuple = new_rel.into_iter().next().unwrap();
        assert_eq!(remaining_tuple.get_typed::<i64>("id").unwrap(), 3);
    }

    #[test]
    fn test_compute_relation_after_delete_no_match() {
        let rel = test_relation();
        let (new_rel, count) =
            compute_relation_after_delete(rel, |t| t.get_typed::<i64>("id").unwrap() == 99)
                .unwrap();

        assert_eq!(count, 0);
        assert_eq!(new_rel.cardinality(), 3);
    }

    #[test]
    fn test_compute_relation_after_update_success() {
        let rel = test_relation();
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 1 || t.get_typed::<i64>("id").unwrap() == 2,
            |t| {
                let mut new_t = t.clone();
                let current_val = new_t.get_typed::<i64>("val").unwrap();
                new_t
                    .set("val".to_string(), ScalarValue::Int(current_val + 100))
                    .unwrap();
                new_t
            },
        )
        .unwrap();

        assert_eq!(count, 2);
        assert_eq!(new_rel.cardinality(), 3);

        // Verify updates
        let mut ids_found = 0;
        for t in new_rel.into_iter() {
            let id = t.get_typed::<i64>("id").unwrap();
            let val = t.get_typed::<i64>("val").unwrap();
            if id == 1 {
                assert_eq!(val, 110);
                ids_found += 1;
            } else if id == 2 {
                assert_eq!(val, 120);
                ids_found += 1;
            } else if id == 3 {
                assert_eq!(val, 30);
                ids_found += 1;
            }
        }
        assert_eq!(ids_found, 3);
    }

    #[test]
    fn test_compute_relation_after_update_merge_identical_tuples() {
        let rel = test_relation();
        // Update tuple with id 2 to be identical to tuple with id 1
        let (new_rel, count) = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |t| {
                let mut new_t = t.clone();
                new_t.set("id".to_string(), ScalarValue::Int(1)).unwrap();
                new_t.set("val".to_string(), ScalarValue::Int(10)).unwrap();
                new_t
            },
        )
        .unwrap();

        assert_eq!(count, 1);
        // Cardinality should reduce by 1 because the updated tuple merges with an existing one
        assert_eq!(new_rel.cardinality(), 2);
    }

    #[test]
    fn test_compute_relation_after_update_mismatch() {
        let rel = test_relation();
        let expected_type = rel.relation_type().tuple_type().clone();
        let res = compute_relation_after_update(
            rel,
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| {
                // Manually create a tuple with wrong type that bypasses the normal setter checks
                // just to test compute_relation_after_update's check
                crate::values::Tuple::new_unchecked(
                    expected_type.clone().into(),
                    std::collections::BTreeMap::from([
                        ("id".to_string(), ScalarValue::Int(1)),
                        ("val".to_string(), ScalarValue::Float(100.0)),
                    ]),
                )
            },
        );

        assert!(matches!(res, Err(DatabaseError::TupleMismatch)));
    }
}
