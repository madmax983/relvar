//! In-memory key index for O(1) key-constraint lookups.
//!
//! The [`Database`](crate::database::Database) maintains a [`KeyIndex`]
//! mapping `(relation name, key attributes)` to the set of key values
//! currently present in the relation. Key validation on insert consults this
//! index instead of loading the whole relation, which keeps insertion O(1)
//! per tuple rather than O(N).
//!
//! The index is:
//! - populated when key constraints are set
//!   ([`Database::set_key_constraints`](crate::database::Database::set_key_constraints)),
//! - updated on every write path ([`Database::insert`](crate::database::Database::insert),
//!   [`Database::bulk_insert`](crate::database::Database::bulk_insert),
//!   [`Database::delete`](crate::database::Database::delete),
//!   [`Database::update`](crate::database::Database::update)),
//! - cleared on [`Database::drop_relvar`](crate::database::Database::drop_relvar),
//! - snapshotted and restored across transactions
//!   ([`Database::begin`](crate::database::Database::begin),
//!   [`Database::rollback`](crate::database::Database::rollback)).
//!
//! This lives at the `Database` layer rather than on the [`StorageEngine`]
//! trait so both the in-memory and persistent engines benefit without any
//! trait changes.

use crate::constraints::{ConstraintManagerError, KeyConstraintError};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::{Relation, ScalarValue, Tuple};
use crate::collections::{HashMap, HashSet};
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// In-memory index of key values per relation.
///
/// Maps `(relation name, key attributes)` to the set of key values present
/// in the relation. Used for O(1) key-constraint lookups on insert.
pub(crate) type KeyIndex = HashMap<(String, Vec<String>), HashSet<Vec<ScalarValue>>>;

/// Key values staged for index insertion: `(key attributes, key values)`.
///
/// Produced during validation and applied to the index only after the
/// corresponding write succeeds, so a failed write cannot leave phantom
/// entries behind.
pub(crate) type StagedKeyValues = Vec<(Vec<String>, Vec<ScalarValue>)>;

/// Extract the key values for `key_attributes` from `tuple`.
///
/// Mirrors [`CandidateKey`](crate::constraints::CandidateKey)'s extraction so
/// index entries compare equal to what constraint validation checks.
fn extract_key_values(
    key_attributes: &[String],
    tuple: &Tuple,
) -> Result<Vec<ScalarValue>, KeyConstraintError> {
    let mut values = Vec::with_capacity(key_attributes.len());
    for attr in key_attributes {
        match tuple.get(attr) {
            Some(value) => values.push(value.clone()),
            None => return Err(KeyConstraintError::TupleMissingAttribute(attr.clone())),
        }
    }
    Ok(values)
}

/// Key attribute lists for a relation, primary key first.
///
/// Returns `None` when the relation has no key constraints.
fn key_attribute_lists<E: StorageEngine>(
    db: &Database<E>,
    relation_name: &str,
) -> Option<Vec<(bool, Vec<String>)>> {
    let constraints = db.constraints.get_key_constraints(relation_name)?;
    let mut lists = Vec::new();
    if let Some(pk) = constraints.primary_key() {
        // `true` marks the primary key for violation error reporting.
        lists.push((true, pk.attributes().to_vec()));
    }
    for ck in constraints.candidate_keys() {
        lists.push((false, ck.attributes().to_vec()));
    }
    Some(lists)
}

/// Map a key-extraction failure the same way
/// [`ConstraintManager::validate_key_constraints_single_tuple`] does.
fn key_extraction_error(error: KeyConstraintError) -> DatabaseError {
    DatabaseError::Constraint(ConstraintManagerError::TransactionError(error.to_string()))
}

impl<E: StorageEngine> Database<E> {
    /// Rebuild the key index entries for a relation from its stored data.
    ///
    /// Drops any existing entries for the relation first, so this also
    /// handles key constraints being replaced.
    pub(crate) fn rebuild_key_index(&mut self, relation_name: &str) -> Result<(), DatabaseError> {
        let relation = self.engine.load_relation(relation_name)?;
        self.rebuild_key_index_from(relation_name, &relation)
    }

    /// Rebuild the key index entries for a relation from an in-hand copy.
    ///
    /// Used after [`delete`](Database::delete)/[`update`](Database::update),
    /// which already hold the post-write relation and would otherwise pay
    /// for a redundant load.
    pub(crate) fn rebuild_key_index_from(
        &mut self,
        relation_name: &str,
        relation: &Relation,
    ) -> Result<(), DatabaseError> {
        self.remove_key_index(relation_name);
        let Some(key_lists) = key_attribute_lists(self, relation_name) else {
            return Ok(());
        };
        for (_, attributes) in &key_lists {
            let mut values = HashSet::with_capacity(relation.cardinality());
            for tuple in relation.tuples() {
                values.insert(extract_key_values(attributes, tuple).map_err(key_extraction_error)?);
            }
            self.key_index
                .insert((relation_name.to_string(), attributes.clone()), values);
        }
        Ok(())
    }

    /// Ensure key index entries exist for every key of the relation,
    /// building them lazily from stored data when missing.
    ///
    /// This is a safety net: the index is normally built eagerly when key
    /// constraints are set, but a relation whose data changed through a
    /// path that bypassed the index (e.g. direct engine use) heals itself
    /// on the next keyed write instead of validating against stale entries.
    pub(crate) fn ensure_key_index(&mut self, relation_name: &str) -> Result<(), DatabaseError> {
        let missing = match key_attribute_lists(self, relation_name) {
            None => return Ok(()),
            Some(lists) => lists.iter().any(|(_, attributes)| {
                !self
                    .key_index
                    .contains_key(&(relation_name.to_string(), attributes.clone()))
            }),
        };
        if missing {
            self.rebuild_key_index(relation_name)?;
        }
        Ok(())
    }

    /// Check one tuple's key values against the index.
    ///
    /// `batch_sets` tracks key values seen earlier in the current batch (one
    /// set per key, primary key first) so bulk inserts also catch
    /// within-batch duplicates. It is grown as needed, so callers validating
    /// a single tuple can pass an empty vector.
    ///
    /// Returns the extracted key values staged for index insertion. The
    /// caller must apply them via
    /// [`apply_staged_key_values`](Self::apply_staged_key_values) only after
    /// the write succeeds.
    pub(crate) fn check_key_values_indexed(
        &mut self,
        relation_name: &str,
        tuple: &Tuple,
        batch_sets: &mut Vec<HashSet<Vec<ScalarValue>>>,
    ) -> Result<StagedKeyValues, DatabaseError> {
        let Some(key_lists) = key_attribute_lists(self, relation_name) else {
            return Ok(Vec::new());
        };
        self.ensure_key_index(relation_name)?;
        while batch_sets.len() < key_lists.len() {
            batch_sets.push(HashSet::new());
        }

        let mut staged = Vec::with_capacity(key_lists.len());
        for ((is_primary, attributes), seen) in key_lists.iter().zip(batch_sets.iter_mut()) {
            let values = extract_key_values(attributes, tuple).map_err(key_extraction_error)?;
            let index_key = (relation_name.to_string(), attributes.clone());
            // `ensure_key_index` guarantees the entry exists.
            debug_assert!(self.key_index.contains_key(&index_key));
            let dominated = self
                .key_index
                .get(&index_key)
                .is_some_and(|existing| existing.contains(&values))
                // `HashSet::insert` returns false when the value was present.
                || !seen.insert(values.clone());
            if dominated {
                return Err(if *is_primary {
                    DatabaseError::Constraint(ConstraintManagerError::PrimaryKeyViolation)
                } else {
                    DatabaseError::Constraint(ConstraintManagerError::CandidateKeyViolation)
                });
            }
            staged.push((attributes.clone(), values));
        }
        Ok(staged)
    }

    /// Apply staged key values to the index after a successful write.
    ///
    /// Must only be called once the corresponding tuples are stored; never
    /// call it for a write that failed, or the index will hold phantom
    /// entries.
    pub(crate) fn apply_staged_key_values(&mut self, relation_name: &str, staged: StagedKeyValues) {
        for (attributes, values) in staged {
            self.key_index
                .entry((relation_name.to_string(), attributes))
                .or_default()
                .insert(values);
        }
    }

    /// Drop all key index entries for a relation.
    pub(crate) fn remove_key_index(&mut self, relation_name: &str) {
        self.key_index.retain(|(name, _), _| name != relation_name);
    }
}
