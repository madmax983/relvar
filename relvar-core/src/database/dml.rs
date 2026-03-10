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
    let relation_type = current_relation.relation_type().clone();

    // Optimization: Pass the iterator directly to `from_tuples_unchecked`
    // instead of collecting into an intermediate `Vec`. Since the source
    // relation is valid, the filtered tuples are also guaranteed to be valid,
    // allowing us to bypass redundant type checking.
    let new_relation = Relation::from_tuples_unchecked(
        relation_type,
        current_relation
            .into_iter()
            .filter(|tuple| !predicate(tuple)),
    );

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
    let expected_type = relation_type.tuple_type().clone();
    let initial_cardinality = current_relation.cardinality();

    let (tuples, update_count) = current_relation.into_iter().try_fold(
        (Vec::with_capacity(initial_cardinality), 0),
        |(mut acc, count), tuple| {
            if !predicate(&tuple) {
                acc.push(tuple);
                return Ok((acc, count));
            }

            let updated_tuple = updater(&tuple);

            if !updated_tuple.conforms_to(&expected_type) {
                return Err(DatabaseError::TupleMismatch);
            }

            acc.push(updated_tuple);
            Ok((acc, count + 1))
        },
    )?;

    // Optimization: The updated tuples are already validated via `conforms_to`
    // inside the `try_fold` loop. We can safely use `from_tuples_unchecked`
    // to build the new relation, avoiding a second O(N*M) validation pass.
    let new_relation = Relation::from_tuples_unchecked(relation_type, tuples);

    Ok((new_relation, update_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::ScalarValue;
    use std::sync::Arc;

    fn get_test_relation_type() -> RelationType {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        RelationType::new(heading)
    }

    fn get_test_relation() -> Relation {
        let mut rel = Relation::new(get_test_relation_type());
        rel.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
        rel.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
        rel.insert(tuple! { id: 3i64, name: "Charlie" }).unwrap();
        rel
    }

    #[test]
    fn test_compute_relation_after_delete_all() {
        let rel = get_test_relation();
        let (new_rel, delete_count) = compute_relation_after_delete(rel, |_| true).unwrap();

        assert_eq!(delete_count, 3);
        assert_eq!(new_rel.cardinality(), 0);
    }

    #[test]
    fn test_compute_relation_after_delete_none() {
        let rel = get_test_relation();
        let (new_rel, delete_count) = compute_relation_after_delete(rel, |_| false).unwrap();

        assert_eq!(delete_count, 0);
        assert_eq!(new_rel.cardinality(), 3);
    }

    #[test]
    fn test_compute_relation_after_delete_some() {
        let rel = get_test_relation();
        let (new_rel, delete_count) =
            compute_relation_after_delete(rel, |t| t.get("id") == Some(&ScalarValue::Int(2)))
                .unwrap();

        assert_eq!(delete_count, 1);
        assert_eq!(new_rel.cardinality(), 2);
        assert!(new_rel.contains(&tuple! { id: 1i64, name: "Alice" }));
        assert!(!new_rel.contains(&tuple! { id: 2i64, name: "Bob" }));
        assert!(new_rel.contains(&tuple! { id: 3i64, name: "Charlie" }));
    }

    #[test]
    fn test_compute_relation_after_update_all() {
        let rel = get_test_relation();
        let rel_type = rel.relation_type().clone();
        let heading_arc = Arc::new(rel_type.tuple_type().clone());

        let (new_rel, update_count) = compute_relation_after_update(
            rel,
            |_| true,
            |t| {
                let mut values = t.clone().values().clone();
                values.insert(
                    "name".to_string(),
                    ScalarValue::String("Updated".to_string()),
                );
                Tuple::new_unchecked(heading_arc.clone(), values)
            },
        )
        .unwrap();

        assert_eq!(update_count, 3);
        assert_eq!(new_rel.cardinality(), 3);
        assert!(new_rel.contains(&tuple! { id: 1i64, name: "Updated" }));
        assert!(new_rel.contains(&tuple! { id: 2i64, name: "Updated" }));
        assert!(new_rel.contains(&tuple! { id: 3i64, name: "Updated" }));
    }

    #[test]
    fn test_compute_relation_after_update_none() {
        let rel = get_test_relation();
        let (new_rel, update_count) =
            compute_relation_after_update(rel.clone(), |_| false, |t| t.clone()).unwrap();

        assert_eq!(update_count, 0);
        assert_eq!(new_rel, rel);
    }

    #[test]
    fn test_compute_relation_after_update_some() {
        let rel = get_test_relation();
        let rel_type = rel.relation_type().clone();
        let heading_arc = Arc::new(rel_type.tuple_type().clone());

        let (new_rel, update_count) = compute_relation_after_update(
            rel,
            |t| t.get("id") == Some(&ScalarValue::Int(2)),
            |t| {
                let mut values = t.clone().values().clone();
                values.insert(
                    "name".to_string(),
                    ScalarValue::String("Robert".to_string()),
                );
                Tuple::new_unchecked(heading_arc.clone(), values)
            },
        )
        .unwrap();

        assert_eq!(update_count, 1);
        assert_eq!(new_rel.cardinality(), 3);
        assert!(new_rel.contains(&tuple! { id: 1i64, name: "Alice" }));
        assert!(new_rel.contains(&tuple! { id: 2i64, name: "Robert" }));
        assert!(new_rel.contains(&tuple! { id: 3i64, name: "Charlie" }));
    }

    #[test]
    fn test_compute_relation_after_update_type_mismatch() {
        let rel = get_test_relation();

        let result = compute_relation_after_update(
            rel,
            |_| true,
            |t| {
                let mut values = t.clone().values().clone();
                // Wrong type for name (Int instead of String)
                values.insert("name".to_string(), ScalarValue::Int(42));
                // We construct the tuple directly to bypass `Tuple::new` validation
                // in order to test `compute_relation_after_update`'s explicit `conforms_to` check
                let bad_heading = TupleType::new()
                    .with_attribute("id", ScalarType::Int)
                    .with_attribute("name", ScalarType::Int);
                Tuple::new_unchecked(Arc::new(bad_heading), values)
            },
        );

        assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
    }
}
