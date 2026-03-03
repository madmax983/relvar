//! Pure Data Manipulation Language (DML) logic for relations.
//!
//! This module contains the core computation logic for `DELETE` and `UPDATE`
//! operations. By extracting this logic from the main `Database` struct,
//! we achieve a cleaner separation of concerns and simpler unit testing.

use crate::error::DatabaseError;
use crate::values::{Relation, Tuple};

/// Computes a new relation after applying a delete operation.
///
/// Tuples matching the given predicate are filtered out.
///
/// # Arguments
///
/// * `current_relation` - The relation to delete tuples from.
/// * `predicate` - A closure that returns `true` for tuples that should be deleted.
///
/// # Returns
///
/// A tuple containing:
/// 1. The resulting `Relation` after the deletions.
/// 2. The number of tuples that were deleted.
pub(crate) fn compute_relation_after_delete<F>(
    current_relation: Relation,
    predicate: F,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
{
    let initial_cardinality = current_relation.cardinality();
    let relation_type = current_relation.relation_type().clone();

    let kept_tuples: Vec<Tuple> = current_relation
        .into_iter()
        .filter(|tuple| !predicate(tuple))
        .collect();

    let delete_count = initial_cardinality - kept_tuples.len();

    let new_relation = Relation::from_tuples(relation_type, kept_tuples)?;

    Ok((new_relation, delete_count))
}

/// Computes a new relation after applying an update operation.
///
/// Tuples matching the given predicate are transformed by the `updater` function.
/// The updated tuples must still conform to the original relation's heading.
/// This function uses an optimized `try_fold` with a guard clause to perform
/// the update and validation in a single pass.
///
/// # Arguments
///
/// * `current_relation` - The relation to update.
/// * `predicate` - A closure that returns `true` for tuples that should be updated.
/// * `updater` - A closure that receives a tuple and returns a modified version of it.
///
/// # Returns
///
/// A tuple containing:
/// 1. The resulting `Relation` after the updates.
/// 2. The number of tuples that were updated.
///
/// # Errors
///
/// Returns `DatabaseError::TupleMismatch` if the updated tuple does not conform
/// to the relation's heading.
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

    let new_relation = Relation::from_tuples(relation_type, tuples)?;

    Ok((new_relation, update_count))
}
