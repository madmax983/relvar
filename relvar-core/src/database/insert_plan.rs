//! Prepared per-batch constraint validation for the insert path (v0.11).
//!
//! Two prepared plans hoist every invariant lookup out of the per-tuple
//! validation loop:
//!
//! - [`InsertContentPlan`]: the relation heading (one `get_relation_metadata`
//!   per batch instead of per tuple) and the type/CHECK/foreign-key
//!   definition lookups. Foreign-key referenced relations are loaded lazily
//!   on the first tuple's content check — once per FK per batch instead of
//!   once per tuple per FK — and each load feeds a pre-built probe set that
//!   turns the per-tuple O(R) referenced scan into an O(1) set probe. The
//!   laziness keeps load-failure precedence identical to the old per-tuple
//!   loader (a missing referenced relation still surfaces after the current
//!   tuple's heading/type/CHECK checks).
//! - [`InsertKeyPlan`]: the key attribute lists and key-index lookup keys
//!   (no per-tuple allocation of the `(relation name, key attributes)`
//!   lookup key).
//!
//! Semantics are identical to the unbatched validators: same checks, same
//! per-tuple order (heading, type, CHECK, foreign keys, then keys), same
//! errors. Callers run the content phase for every tuple before the key
//! phase, preserving the previous error precedence.

use crate::collections::{HashMap, HashSet};
use crate::constraints::{
    AttributeConstraints, CheckConstraints, ConstraintManagerError, ForeignKey,
    ForeignKeyConstraints,
};
use crate::database::Database;
use crate::database::key_index::{
    KeyIndex, StagedKeyValues, extract_key_values, key_attribute_lists, key_extraction_error,
};
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::types::RelationType;
use crate::values::{ScalarValue, Tuple};
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// One foreign key with its referenced key combinations pre-built.
///
/// The probe set holds the referenced-attribute value combinations of every
/// tuple in the referenced relation, so per-tuple validation is a single
/// set-membership probe instead of a full relation scan.
struct PreparedForeignKey<'a> {
    foreign_key: &'a ForeignKey,
    referenced_keys: HashSet<Vec<ScalarValue>>,
}

/// Prepared per-batch validation state for heading, type, CHECK, and
/// foreign-key constraints.
///
/// Construct with [`prepare`](Self::prepare), then call
/// [`validate_tuple_content`](Self::validate_tuple_content) per tuple.
/// Same order and errors as `ConstraintManager::validate_tuple_type`
/// followed by `validate_tuple_content_constraints`.
pub(crate) struct InsertContentPlan<'a, E: StorageEngine> {
    engine: &'a E,
    relation_type: RelationType,
    type_constraints: Option<&'a HashMap<String, AttributeConstraints>>,
    check_constraints: Option<&'a CheckConstraints>,
    fk_constraints: Option<&'a ForeignKeyConstraints>,
    prepared_fks: Vec<PreparedForeignKey<'a>>,
    fks_prepared: bool,
}

impl<'a, E: StorageEngine> InsertContentPlan<'a, E> {
    /// Prepare content validation for inserting into `relation_name`.
    ///
    /// Fetches the heading once and resolves the type/CHECK/foreign-key
    /// definitions once. Referenced relations are loaded on the first
    /// [`validate_tuple_content`](Self::validate_tuple_content) call.
    pub(crate) fn prepare(db: &'a Database<E>, relation_name: &str) -> Result<Self, DatabaseError> {
        let metadata = db.engine.get_relation_metadata(relation_name)?;
        Ok(Self {
            engine: &db.engine,
            relation_type: metadata.relation_type,
            type_constraints: db.constraints.get_type_constraints(relation_name),
            check_constraints: db.constraints.get_check_constraints(relation_name),
            fk_constraints: db.constraints.get_foreign_key_constraints(relation_name),
            prepared_fks: Vec::new(),
            fks_prepared: false,
        })
    }

    /// Validate heading conformance, type constraints, CHECK constraints,
    /// and foreign keys for one tuple.
    pub(crate) fn validate_tuple_content(&mut self, tuple: &Tuple) -> Result<(), DatabaseError> {
        if !tuple.conforms_to(self.relation_type.tuple_type()) {
            return Err(DatabaseError::Constraint(
                ConstraintManagerError::TupleMismatch,
            ));
        }

        if let Some(attr_constraints) = self.type_constraints {
            for (attr_name, constraints) in attr_constraints {
                if let Some(value) = tuple.get(attr_name)
                    && !constraints.is_satisfied_by(value).map_err(|e| {
                        ConstraintManagerError::TypeConstraintViolation(e.to_string())
                    })?
                {
                    return Err(DatabaseError::Constraint(
                        ConstraintManagerError::TypeConstraintViolation(format!(
                            "Attribute {} violates constraint",
                            attr_name
                        )),
                    ));
                }
            }
        }

        if let Some(check_constraints) = self.check_constraints {
            check_constraints.are_all_satisfied_by(tuple).map_err(|e| {
                DatabaseError::Constraint(ConstraintManagerError::CheckConstraintViolation(e))
            })?;
        }

        // Load each referenced relation once per batch, on first use, and
        // build one probe set per foreign key. Lazy so a load failure
        // surfaces exactly where the old per-tuple loader raised it.
        if !self.fks_prepared {
            self.fks_prepared = true;
            if let Some(fk_constraints) = self.fk_constraints {
                let mut prepared = Vec::with_capacity(fk_constraints.foreign_keys().len());
                for fk in fk_constraints.foreign_keys() {
                    let referenced = self.engine.load_relation(fk.referenced_relation_name())?;
                    let referenced_keys = fk
                        .referenced_key_set(&referenced)
                        .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?;
                    prepared.push(PreparedForeignKey {
                        foreign_key: fk,
                        referenced_keys,
                    });
                }
                self.prepared_fks = prepared;
            }
        }

        for prepared in &self.prepared_fks {
            if prepared
                .foreign_key
                .would_violate_against_set(tuple, &prepared.referenced_keys)
                .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?
            {
                return Err(DatabaseError::Constraint(
                    ConstraintManagerError::ForeignKeyViolation(
                        "Foreign key constraint violated".to_string(),
                    ),
                ));
            }
        }

        Ok(())
    }
}

/// One key constraint with its index lookup key precomputed.
struct PlannedKey {
    /// Whether this is the primary key (selects the violation error).
    is_primary: bool,
    /// Key attributes, cloned once per batch.
    attributes: Vec<String>,
    /// Precomputed `(relation name, key attributes)` index lookup key.
    index_key: (String, Vec<String>),
}

impl PlannedKey {
    fn new(relation_name: &str, is_primary: bool, attributes: &[String]) -> Self {
        let attributes = attributes.to_vec();
        Self {
            is_primary,
            index_key: (relation_name.to_string(), attributes.clone()),
            attributes,
        }
    }
}

/// Prepared per-batch validation state for key constraints.
///
/// Fully owned (no borrows): build with [`prepare`](Self::prepare) after
/// the content phase and after warming the key index, then call
/// [`validate_tuple_keys`](Self::validate_tuple_keys) per tuple. Same
/// errors as the previous per-tuple key validation.
pub(crate) struct InsertKeyPlan {
    planned_keys: Vec<PlannedKey>,
}

impl InsertKeyPlan {
    /// Precompute the key attribute lists and index lookup keys for
    /// `relation_name`. The caller must have warmed the key index first
    /// (see `ensure_key_index`); no writes may happen between warming and
    /// the per-tuple checks.
    pub(crate) fn prepare<E: StorageEngine>(db: &Database<E>, relation_name: &str) -> Self {
        let mut planned_keys = Vec::new();
        // Primary key first, then candidate keys — the same ordering the
        // previous per-tuple validation used.
        if let Some(key_lists) = key_attribute_lists(db, relation_name) {
            for (is_primary, attributes) in key_lists {
                planned_keys.push(PlannedKey::new(relation_name, is_primary, &attributes));
            }
        }
        Self { planned_keys }
    }

    /// Validate key constraints for one tuple against the key index.
    ///
    /// `batch_sets` tracks key values seen earlier in the current batch (one
    /// set per key) so bulk inserts still catch within-batch duplicates.
    ///
    /// Returns the extracted key values staged for index insertion; the
    /// caller must apply them only after the corresponding write succeeds.
    pub(crate) fn validate_tuple_keys(
        &self,
        key_index: &KeyIndex,
        tuple: &Tuple,
        batch_sets: &mut Vec<HashSet<Vec<ScalarValue>>>,
    ) -> Result<StagedKeyValues, DatabaseError> {
        while batch_sets.len() < self.planned_keys.len() {
            batch_sets.push(HashSet::new());
        }

        let mut staged = Vec::with_capacity(self.planned_keys.len());
        for (planned, seen) in self.planned_keys.iter().zip(batch_sets.iter_mut()) {
            let values =
                extract_key_values(&planned.attributes, tuple).map_err(key_extraction_error)?;
            // The caller warmed the index before the key phase, so the
            // entry exists.
            debug_assert!(key_index.contains_key(&planned.index_key));
            let dominated = key_index
                .get(&planned.index_key)
                .is_some_and(|existing| existing.contains(&values))
                || !seen.insert(values.clone());
            if dominated {
                return Err(if planned.is_primary {
                    DatabaseError::Constraint(ConstraintManagerError::PrimaryKeyViolation)
                } else {
                    DatabaseError::Constraint(ConstraintManagerError::CandidateKeyViolation)
                });
            }
            staged.push((planned.attributes.clone(), values));
        }
        Ok(staged)
    }
}
