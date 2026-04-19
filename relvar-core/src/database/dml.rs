//! Data Manipulation Language (DML) primitives.
//!
//! This module contains pure functions for computing the resulting state of a relation
//! after applying `UPDATE` or `DELETE` operations. These functions are intentionally decoupled
//! from the `Database` struct and storage engine to facilitate testing and optimize
//! tuple evaluation loops.
use crate::error::DatabaseError;
use crate::values::{Relation, Tuple};

/// Computes the relation after applying a DELETE operation.
///
/// Optimization (Bolt): We use `restrict_into` to filter the existing `Relation` in place.
/// This calls `HashSet::retain()` internally, keeping the allocated memory
/// and avoiding the overhead of generating a new wrapper, re-hashing tuples,
/// and collecting them into a new internal HashSet.
/// Reduces memory allocations drastically for large DELETE operations.
pub(crate) fn compute_relation_after_delete<F>(
    current_relation: Relation,
    mut predicate: F,
) -> Result<(Relation, usize), DatabaseError>
where
    F: FnMut(&Tuple) -> bool,
{
    let initial_cardinality = current_relation.cardinality();

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
