//! The Golden Rule (TTM RM Prescription 23).
//!
//! Every database constraint must be satisfied at every statement boundary.
//! This module holds the whole-database integrity check
//! ([`Database::verify_integrity`]) and the shared helpers that validate a
//! relvar value and a foreign key against explicit relation values, so the
//! statement paths can validate a *candidate* post-statement state before it
//! is written.

use crate::constraints::assertion::AssertionError;
use crate::constraints::{ConstraintManagerError, ForeignKey};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::Relation;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// Builds the foreign-key violation error for `fk` declared on `referencing`.
pub(crate) fn foreign_key_violation(referencing: &str, fk: &ForeignKey) -> DatabaseError {
    DatabaseError::Constraint(ConstraintManagerError::ForeignKeyViolation(format!(
        "{}({}) -> {}({}) is not satisfied",
        referencing,
        fk.foreign_key_attributes().join(", "),
        fk.referenced_relation_name(),
        fk.referenced_attributes().join(", "),
    )))
}

impl<E: StorageEngine> Database<E> {
    /// Checks every database constraint against the current state.
    ///
    /// TTM: RM Prescription 23 (the Golden Rule) requires every constraint
    /// to hold at every statement boundary. The statement paths
    /// ([`insert`](Self::insert), [`update`](Self::update),
    /// [`delete`](Self::delete), ...) enforce that incrementally; this method re-derives it from
    /// scratch, which makes it useful after recovery, after importing data
    /// written by other tools, or as a test oracle.
    ///
    /// Checked, in order:
    /// 1. every declared constraint names a base relvar that exists;
    /// 2. per base relvar: every tuple conforms to the relvar's heading,
    ///    then key (primary and candidate) uniqueness, type constraints and
    ///    CHECK constraints;
    /// 3. every foreign key;
    /// 4. every registered [`DatabaseAssertion`](crate::DatabaseAssertion).
    ///
    /// # Performance
    ///
    /// O(total database size): every base relvar is loaded once, and each
    /// foreign key loads its two relvars once.
    ///
    /// # Errors
    ///
    /// Returns the first violation found:
    /// [`DatabaseError::RelationNotFound`] for a constraint on a missing
    /// relvar, [`ConstraintManagerError::TupleMismatch`],
    /// [`ConstraintManagerError::PrimaryKeyViolation`],
    /// [`ConstraintManagerError::CandidateKeyViolation`],
    /// [`ConstraintManagerError::TypeConstraintViolation`],
    /// [`ConstraintManagerError::CheckConstraintViolation`],
    /// [`ConstraintManagerError::ForeignKeyViolation`] (all wrapped in
    /// [`DatabaseError::Constraint`]) or
    /// [`DatabaseError::AssertionViolation`]; or a storage error if a relvar
    /// cannot be loaded.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.set_key_constraints(
    ///     "USERS",
    ///     KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".to_string()]).unwrap()),
    /// ).unwrap();
    /// db.insert("USERS", tuple! { id: 1i64 }).unwrap();
    ///
    /// assert!(db.verify_integrity().is_ok());
    /// ```
    pub fn verify_integrity(&mut self) -> Result<(), DatabaseError> {
        // 1. No constraint may outlive (or predate) its relvar.
        let dangling = self
            .constraints
            .constrained_relations()
            .chain(
                self.constraints
                    .all_foreign_keys()
                    .map(|(_, fk)| fk.referenced_relation_name()),
            )
            .find(|name| !self.engine.relation_exists(name))
            .map(ToString::to_string);
        if let Some(name) = dangling {
            return Err(DatabaseError::RelationNotFound(name));
        }

        // 2. Per-relvar constraints, in a deterministic order.
        let mut names = self.engine.list_relations();
        names.sort();
        for name in &names {
            let relation = self.engine.load_relation(name)?;
            self.validate_relvar_value(name, &relation, true)?;
        }

        // 3. Foreign keys.
        let foreign_keys: Vec<(String, ForeignKey)> = self
            .constraints
            .all_foreign_keys()
            .map(|(name, fk)| (name.to_string(), fk.clone()))
            .collect();
        for (referencing, fk) in &foreign_keys {
            let referencing_relation = self.engine.load_relation(referencing)?;
            let referenced_relation = self.engine.load_relation(fk.referenced_relation_name())?;
            check_foreign_key(referencing, fk, &referencing_relation, &referenced_relation)?;
        }

        // 4. Database assertions.
        self.evaluate_assertions()?;
        Ok(())
    }

    /// Validates a (candidate) value for base relvar `name` against every
    /// constraint local to that relvar: heading, keys, type constraints and
    /// CHECK constraints. Foreign keys are validated separately because they
    /// span relvars.
    ///
    /// `check_tuples_conform` additionally checks every tuple against the
    /// heading; callers that built `relation` from already-checked tuples
    /// may skip that O(N) pass.
    pub(crate) fn validate_relvar_value(
        &self,
        name: &str,
        relation: &Relation,
        check_tuples_conform: bool,
    ) -> Result<(), DatabaseError> {
        let heading = self.engine.get_relation_metadata(name)?.relation_type;
        if relation.relation_type() != &heading {
            return Err(ConstraintManagerError::TupleMismatch.into());
        }
        if check_tuples_conform
            && relation
                .tuples()
                .any(|tuple| !tuple.conforms_to(heading.tuple_type()))
        {
            return Err(ConstraintManagerError::TupleMismatch.into());
        }

        if let Some(keys) = self.constraints.get_key_constraints(name) {
            self.constraints
                .validate_key_constraints_bulk(relation, keys)?;
        }
        for tuple in relation.tuples() {
            self.constraints.validate_type_constraints(name, tuple)?;
            self.constraints.validate_check_constraints(name, tuple)?;
        }
        Ok(())
    }

    /// Evaluates every registered database assertion against the current
    /// state, without any rollback side effects.
    ///
    /// Returns the first violated assertion as an error.
    pub(crate) fn evaluate_assertions(&mut self) -> Result<(), AssertionError> {
        if self.assertions.is_empty() {
            return Ok(());
        }
        // Move the assertions out of `self` while evaluating: predicates take
        // `&mut Database<E>` (they query relvars), which cannot coexist with
        // an outstanding borrow of `self.assertions`.
        let assertions = core::mem::take(&mut self.assertions);
        let violated = assertions
            .iter()
            .find(|assertion| !assertion.is_satisfied_by(self))
            .map(|assertion| AssertionError::Violation {
                assertion_name: assertion.name().to_string(),
                description: assertion.description().to_string(),
            });
        // Always restore the assertion list, even on violation.
        self.assertions = assertions;
        violated.map_or(Ok(()), Err)
    }
}

/// Checks one foreign key declared on `referencing` against explicit
/// referencing and referenced relation values.
///
/// Taking the values explicitly (rather than loading them) is what lets the
/// statement paths check a candidate post-statement state, including
/// self-referencing foreign keys where both values are the same relvar.
pub(crate) fn check_foreign_key(
    referencing: &str,
    fk: &ForeignKey,
    referencing_relation: &Relation,
    referenced_relation: &Relation,
) -> Result<(), DatabaseError> {
    let satisfied = fk
        .is_satisfied_by(referencing_relation, referenced_relation)
        .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?;
    if satisfied {
        Ok(())
    } else {
        Err(foreign_key_violation(referencing, fk))
    }
}
