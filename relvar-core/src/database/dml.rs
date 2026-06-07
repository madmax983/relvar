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
    use crate::values::ScalarValue;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn get_tuple(id: i64, val: i64) -> Tuple {
        let mut map = BTreeMap::new();
        map.insert("id".to_string(), ScalarValue::Int(id));
        map.insert("val".to_string(), ScalarValue::Int(val));
        Tuple::new(
            Arc::new(
                TupleType::new()
                    .with_attribute("id", ScalarType::Int)
                    .with_attribute("val", ScalarType::Int),
            ),
            map,
        )
        .unwrap()
    }

    #[test]
    fn should_return_error_when_update_creates_tuple_mismatch() {
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("val", ScalarType::Int),
        );
        let tuple1 = get_tuple(1, 10);
        let mut relation = Relation::new(rel_type);
        relation.insert(tuple1).unwrap();

        let predicate = |t: &Tuple| t.get_typed::<i64>("id").unwrap() == 1;
        let updater = |_t: &Tuple| {
            let mut map = BTreeMap::new();
            map.insert("id".to_string(), ScalarValue::Int(1));
            Tuple::new(
                Arc::new(TupleType::new().with_attribute("id", ScalarType::Int)),
                map,
            )
            .unwrap()
        };

        let result = compute_relation_after_update(relation, predicate, updater);
        assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
    }
}
