//! Database Data Manipulation (DML) and Query operations.

use super::dml::{compute_relation_after_delete, compute_relation_after_update};
use crate::constraints::assertion::AssertionError;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::{Relation, Tuple};

impl<E: StorageEngine> Database<E> {
    /// - Any constraint is violated (Key, Foreign Key, Type, Check)
    /// - A database assertion is violated ([`DatabaseError::AssertionViolation`]);
    ///   the insert is rolled back
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    ///
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    /// ```
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        self.ensure_not_virtual(relation_name)?;
        self.validate_insert(relation_name, &tuple)?;

        // Snapshot the engine so the insert can be rolled back if a database
        // assertion is violated (no-op when no assertions are registered).
        self.begin_assertion_guard()?;

        // Insert into engine, restoring the pre-insert state if it fails.
        if let Err(op_error) = self.engine.insert_tuple(relation_name, tuple) {
            self.abort_assertion_guard()?;
            return Err(op_error.into());
        }

        // Enforce database assertions against the post-insert state,
        // rolling back the insert on violation.
        self.check_assertions()
    }

    /// Query a relation (base or virtual).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    ///
    /// let result = db.query("TEST").unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation doesn't exist.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    ///
    /// let rel = db.query("USERS").unwrap();
    /// assert_eq!(rel.cardinality(), 1);
    /// ```
    pub fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar
        if let Some(virtual_relvar) = self.virtual_relvars.get(relation_name) {
            return (virtual_relvar.evaluator)(self);
        }

        // Otherwise, load from engine
        Ok(self.engine.load_relation(relation_name)?)
    }

    /// Delete tuples matching a predicate.
    ///
    /// Obtains the total count of tuples removed from the relation.
    ///
    /// # Errors
    ///
    /// Yields an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - Deleting the tuples would violate a foreign key constraint in another relation
    ///   ([`crate::constraints::ConstraintManagerError::ForeignKeyViolation`])
    /// - A database assertion is violated ([`DatabaseError::AssertionViolation`]);
    ///   the delete is rolled back
    ///
    /// # Performance
    ///
    /// This operation scans the entire relation to evaluate the predicate.
    /// It then constructs a new relation containing the remaining tuples.
    /// Complexity is O(N) where N is the relation size.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    /// db.insert("TEST", tuple! { id: 2i64 }).unwrap();
    ///
    /// // Delete id 1
    /// let deleted_count = db.delete("TEST", |t| {
    ///     t.get_typed::<i64>("id").unwrap() == 1
    /// }).unwrap();
    ///
    /// assert_eq!(deleted_count, 1);
    /// assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    /// db.insert("USERS", tuple!{ id: 2i64 }).unwrap();
    ///
    /// let deleted_count = db.delete("USERS", |t| t.get_typed::<i64>("id").unwrap() == 1).unwrap();
    /// assert_eq!(deleted_count, 1);
    /// ```
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        self.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Filter out tuples to delete
        let (new_relation, delete_count) =
            compute_relation_after_delete(current_relation, predicate)?;

        self.constraints.validate_referencing_foreign_keys(
            &mut self.engine,
            relation_name,
            &new_relation,
        )?;

        // Snapshot the engine so the delete can be rolled back if a database
        // assertion is violated (no-op when no assertions are registered).
        self.begin_assertion_guard()?;

        // Store the new relation, restoring the pre-delete state if it fails.
        if let Err(op_error) = self.engine.store_relation(relation_name, &new_relation) {
            self.abort_assertion_guard()?;
            return Err(op_error.into());
        }

        // Enforce database assertions against the post-delete state,
        // rolling back the delete on violation.
        self.check_assertions()?;
        Ok(delete_count)
    }

    /// Update tuples matching a predicate.
    ///
    /// Obtains the total count of tuples modified during the operation.
    ///
    /// # Errors
    ///
    /// Yields an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - The updated tuple doesn't match the relation type ([`DatabaseError::TupleMismatch`])
    /// - A key constraint is violated ([`crate::constraints::ConstraintManagerError::PrimaryKeyViolation`], [`crate::constraints::ConstraintManagerError::CandidateKeyViolation`])
    /// - A foreign key constraint is violated ([`crate::constraints::ConstraintManagerError::ForeignKeyViolation`])
    /// - A type constraint is violated ([`crate::constraints::ConstraintManagerError::TypeConstraintViolation`])
    /// - A CHECK constraint is violated ([`crate::constraints::ConstraintManagerError::CheckConstraintViolation`])
    /// - A database assertion is violated ([`DatabaseError::AssertionViolation`]);
    ///   the update is rolled back
    ///
    /// # Performance
    ///
    /// - **Constraint Validation:** Currently, all constraints (CHECK, Type, Foreign Key) are re-validated
    ///   against **every tuple** in the relation after the update, not just the modified ones.
    ///   This ensures total consistency but has O(N) complexity where N is the relation size.
    ///   Future versions may optimize this to O(K) where K is the number of updated tuples.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("salary", ScalarType::Int)
    /// );
    /// db.create_relvar("EMPLOYEES", rel_type).unwrap();
    /// db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 }).unwrap();
    ///
    /// // Give a 10% raise to employee 1
    /// let count = db.update(
    ///     "EMPLOYEES",
    ///     |t| t.get_typed::<i64>("id").unwrap() == 1,
    ///     |t| {
    ///         let old_salary = t.get_typed::<i64>("salary").unwrap();
    ///         tuple! {
    ///             id: t.get_typed::<i64>("id").unwrap(),
    ///             salary: old_salary + (old_salary / 10)
    ///         }
    ///     }
    /// ).unwrap();
    ///
    /// assert_eq!(count, 1);
    /// let employees = db.query("EMPLOYEES").unwrap();
    /// let emp1 = employees.tuples().next().unwrap();
    /// assert_eq!(emp1.get_typed::<i64>("salary").unwrap(), 55000);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    ///
    /// let updated_count = db.update(
    ///     "USERS",
    ///     |t| t.get_typed::<i64>("id").unwrap() == 1,
    ///     |t| { let mut t2 = t.clone(); t2.set("id".to_string(), ScalarValue::Int(2)).unwrap(); t2 }
    /// ).unwrap();
    /// assert_eq!(updated_count, 1);
    /// ```
    pub fn update<F, U>(
        &mut self,
        relation_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
        U: Fn(&Tuple) -> Tuple,
    {
        self.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Apply updates
        let (new_relation, update_count) =
            compute_relation_after_update(current_relation, predicate, updater)?;

        self.validate_relation_constraints(relation_name, &new_relation)?;

        self.constraints.validate_referencing_foreign_keys(
            &mut self.engine,
            relation_name,
            &new_relation,
        )?;

        // Snapshot the engine so the update can be rolled back if a database
        // assertion is violated (no-op when no assertions are registered).
        self.begin_assertion_guard()?;

        // Store the new relation, restoring the pre-update state if it fails.
        if let Err(op_error) = self.engine.store_relation(relation_name, &new_relation) {
            self.abort_assertion_guard()?;
            return Err(op_error.into());
        }

        // Enforce database assertions against the post-update state,
        // rolling back the update on violation.
        self.check_assertions()?;
        Ok(update_count)
    }

    // --- Helper Methods ---

    /// Begins an assertion guard: snapshots the engine before a DML mutation
    /// so the mutation can be rolled back if a database assertion is violated.
    ///
    /// This is a no-op (no snapshot is taken) when no assertions are
    /// registered, keeping assertion-free DML at zero overhead.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::TransactionError`] if a guard is already
    /// active (assertion predicates must not perform DML), or a
    /// [`DatabaseError::Storage`] error if the engine cannot snapshot.
    fn begin_assertion_guard(&mut self) -> Result<(), DatabaseError> {
        if self.assertions.is_empty() {
            return Ok(());
        }
        if self.assertion_snapshot.is_some() {
            return Err(DatabaseError::TransactionError(
                "assertion guard already active: assertion predicates must not perform DML"
                    .to_string(),
            ));
        }
        let snapshot = self.engine.begin_transaction()?;
        self.assertion_snapshot = Some(snapshot);
        Ok(())
    }

    /// Aborts an assertion guard after the guarded operation itself failed:
    /// restores the pre-operation engine state and discards the snapshot.
    ///
    /// # Errors
    ///
    /// Returns a [`DatabaseError::Storage`] error if the engine cannot roll back.
    fn abort_assertion_guard(&mut self) -> Result<(), DatabaseError> {
        if let Some(snapshot) = self.assertion_snapshot.take() {
            self.engine.rollback_transaction(snapshot)?;
        }
        Ok(())
    }

    /// Checks every registered database assertion against the current state.
    ///
    /// Called after each DML mutation. If any assertion is violated, the
    /// guarded operation is rolled back and
    /// [`DatabaseError::AssertionViolation`] is returned; otherwise the guard
    /// snapshot is committed and `Ok(())` is returned.
    ///
    /// When no assertions are registered this is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::AssertionViolation`] naming the first violated
    /// assertion, or a [`DatabaseError::Storage`] error if the engine cannot
    /// roll back or commit the guard snapshot.
    fn check_assertions(&mut self) -> Result<(), DatabaseError> {
        if self.assertions.is_empty() {
            return Ok(());
        }

        // Move the assertions out of `self` while evaluating: predicates take
        // `&mut Database<E>` (they query relvars), which cannot coexist with
        // an outstanding borrow of `self.assertions`.
        let assertions = std::mem::take(&mut self.assertions);
        let mut violated: Option<AssertionError> = None;
        for assertion in &assertions {
            if !assertion.is_satisfied_by(self) {
                violated = Some(AssertionError::Violation {
                    assertion_name: assertion.name().to_string(),
                    description: assertion.description().to_string(),
                });
                break;
            }
        }
        // Always restore the assertion list, even on violation.
        self.assertions = assertions;

        match violated {
            Some(assertion_error) => {
                // Roll back the operation that triggered the violation.
                if let Some(snapshot) = self.assertion_snapshot.take() {
                    self.engine.rollback_transaction(snapshot)?;
                }
                Err(assertion_error.into())
            }
            None => {
                // All assertions hold: release the pre-operation snapshot.
                if let Some(snapshot) = self.assertion_snapshot.take() {
                    self.engine.commit_transaction(snapshot)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn ensure_not_virtual(&self, relation_name: &str) -> Result<(), DatabaseError> {
        if self.virtual_relvars.contains_key(relation_name) {
            Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn validate_insert(
        &mut self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        // Pure checks and Type validations
        self.constraints
            .validate_tuple_type(&self.engine, relation_name, tuple)?;

        self.constraints.validate_tuple_content_constraints(
            &mut self.engine,
            relation_name,
            tuple,
        )?;

        // Load current relation to check key constraints
        let current_relation = self.query(relation_name)?;
        self.constraints.validate_key_constraints_single_tuple(
            relation_name,
            tuple,
            &current_relation,
        )?;

        Ok(())
    }

    pub(crate) fn validate_relation_constraints(
        &mut self,
        relation_name: &str,
        relation: &Relation,
    ) -> Result<(), DatabaseError> {
        // Validate key constraints on new relation
        if let Some(key_constraints) = self.constraints.get_key_constraints(relation_name) {
            self.constraints
                .validate_key_constraints_bulk(relation, key_constraints)?;
        }

        // Validate other constraints (Type, CHECK, FK) on all tuples in the new relation
        // NOTE: In a production system we'd only validate changed tuples, but for now
        // we validate everything to ensure total consistency.
        self.constraints.validate_tuple_content_constraints_bulk(
            &mut self.engine,
            relation_name,
            relation,
        )?;

        Ok(())
    }
}
