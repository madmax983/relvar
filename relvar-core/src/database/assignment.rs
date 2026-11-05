//! Multiple assignment (TTM RM Prescription 21).
//!
//! A multiple assignment assigns new values to several relvars as a single
//! statement. The Golden Rule (RM Prescription 23) applies at statement
//! boundaries only, so constraints spanning the assigned relvars are
//! checked on the combined final state — which is what makes it possible
//! to populate (or empty) relvars whose foreign keys reference each other.

use crate::database::Database;
use crate::database::golden_rule::ForeignKeyScope;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::Relation;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

impl<E: StorageEngine> Database<E> {
    /// Assigns new values to several base relvars as one statement
    /// (multiple assignment).
    ///
    /// TTM: RM Prescription 21 — multiple assignment. Every `(relvar,
    /// value)` pair replaces that relvar's value, and the statement is
    /// atomic: every constraint is checked against the *final* state with
    /// all assignments applied, and either every assignment takes effect or
    /// none does. Constraints checked:
    ///
    /// 1. each value's heading is the relvar's heading, and every tuple
    ///    conforms to it;
    /// 2. each relvar's keys, type constraints and CHECK constraints;
    /// 3. every foreign key out of or into an assigned relvar, evaluated on
    ///    the new value of each assigned side and the stored value of an
    ///    unassigned side;
    /// 4. every registered database assertion (after the writes, which are
    ///    undone on violation).
    ///
    /// Because the intermediate states are never checked, multiple
    /// assignment can populate relvars whose foreign keys reference each
    /// other, which no sequence of single-relvar statements can.
    ///
    /// Works both outside and inside a transaction. Outside one, the
    /// statement runs in its own engine transaction. Inside one, a failed
    /// statement is undone without ending the transaction, and a later
    /// [`rollback`](Self::rollback) undoes a successful one like any other
    /// statement.
    ///
    /// An empty list of assignments is a no-op.
    ///
    /// # Errors
    ///
    /// Nothing is changed when an error is returned.
    ///
    /// - [`DatabaseError::CannotModifyVirtualRelvar`] if a target is a
    ///   virtual relvar;
    /// - [`DatabaseError::RelationNotFound`] if a target does not exist;
    /// - [`DatabaseError::DuplicateAssignmentTarget`] if a relvar is named
    ///   more than once;
    /// - [`DatabaseError::Constraint`] wrapping
    ///   [`TupleMismatch`](crate::constraints::ConstraintManagerError::TupleMismatch)
    ///   (heading mismatch),
    ///   [`PrimaryKeyViolation`](crate::constraints::ConstraintManagerError::PrimaryKeyViolation),
    ///   [`CandidateKeyViolation`](crate::constraints::ConstraintManagerError::CandidateKeyViolation),
    ///   [`TypeConstraintViolation`](crate::constraints::ConstraintManagerError::TypeConstraintViolation),
    ///   [`CheckConstraintViolation`](crate::constraints::ConstraintManagerError::CheckConstraintViolation)
    ///   or [`ForeignKeyViolation`](crate::constraints::ConstraintManagerError::ForeignKeyViolation);
    /// - [`DatabaseError::AssertionViolation`] if a database assertion does
    ///   not hold on the final state;
    /// - [`DatabaseError::TransactionError`] from inside an assertion
    ///   predicate (predicates must be read-only);
    /// - a storage error if the engine cannot write a relvar.
    ///
    /// # Examples
    ///
    /// Two relvars that reference each other can only be populated
    /// together:
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let emp = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("emp_id", ScalarType::Int)
    ///         .with_attribute("dept_id", ScalarType::Int),
    /// );
    /// let dept = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("dept_id", ScalarType::Int)
    ///         .with_attribute("manager", ScalarType::Int),
    /// );
    /// db.create_relvar("EMP", emp.clone()).unwrap();
    /// db.create_relvar("DEPT", dept.clone()).unwrap();
    /// let key = |a: &str| KeyConstraints::new().with_primary_key(PrimaryKey::new(vec![a.to_string()]).unwrap());
    /// let fk = |from: &str, to: &str, attr: &str| ForeignKeyConstraints::new().with_foreign_key(
    ///     ForeignKey::new(vec![from.to_string()], to.to_string(), vec![attr.to_string()]).unwrap(),
    /// );
    /// db.set_key_constraints("EMP", key("emp_id")).unwrap();
    /// db.set_key_constraints("DEPT", key("dept_id")).unwrap();
    /// // Every employee works in a department; every department's manager is an employee.
    /// db.set_foreign_key_constraints("EMP", fk("dept_id", "DEPT", "dept_id")).unwrap();
    /// db.set_foreign_key_constraints("DEPT", fk("manager", "EMP", "emp_id")).unwrap();
    ///
    /// // Neither single insert can succeed first...
    /// assert!(db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64 }).is_err());
    /// // ...but both assignments together can.
    /// db.multiple_assign(vec![
    ///     ("EMP", Relation::from_tuples(emp, vec![tuple! { emp_id: 1i64, dept_id: 10i64 }]).unwrap()),
    ///     ("DEPT", Relation::from_tuples(dept, vec![tuple! { dept_id: 10i64, manager: 1i64 }]).unwrap()),
    /// ])
    /// .unwrap();
    /// assert_eq!(db.query("EMP").unwrap().cardinality(), 1);
    /// ```
    pub fn multiple_assign<N>(
        &mut self,
        assignments: impl IntoIterator<Item = (N, Relation)>,
    ) -> Result<(), DatabaseError>
    where
        N: Into<String>,
    {
        let assignments: Vec<(String, Relation)> = assignments
            .into_iter()
            .map(|(name, relation)| (name.into(), relation))
            .collect();
        if assignments.is_empty() {
            return Ok(());
        }

        // 1. Every target is a distinct, existing base relvar.
        for (position, (name, _)) in assignments.iter().enumerate() {
            self.ensure_not_virtual(name)?;
            if !self.engine.relation_exists(name) {
                return Err(DatabaseError::RelationNotFound(name.clone()));
            }
            if assignments[..position]
                .iter()
                .any(|(earlier, _)| earlier == name)
            {
                return Err(DatabaseError::DuplicateAssignmentTarget(name.to_string()));
            }
        }

        // 2. Golden Rule on the final state: each new value against its
        //    relvar's own constraints, then every foreign key touching an
        //    assigned relvar, with all assignments applied at once.
        for (name, relation) in &assignments {
            self.validate_relvar_value(name, relation, true)?;
        }
        let changes: Vec<(&str, &Relation)> = assignments
            .iter()
            .map(|(name, relation)| (name.as_str(), relation))
            .collect();
        self.validate_foreign_keys_for_changes(&changes, ForeignKeyScope::All)?;

        // 3. Write every assignment as one guarded statement: several
        //    writes need engine-level atomicity even without assertions.
        let names: Vec<&str> = assignments.iter().map(|(name, _)| name.as_str()).collect();
        let guard = self.begin_statement_guard(&names, assignments.len() > 1)?;
        for (name, relation) in &assignments {
            if let Err(write_error) = self.engine.store_relation(name, relation) {
                self.abort_statement(guard)?;
                return Err(write_error.into());
            }
        }
        // Database assertions see the final state; a violation undoes
        // every write.
        self.finish_statement(guard)?;

        // 4. The statement took effect: refresh the key index (#23) from
        //    the values in hand.
        for (name, relation) in &assignments {
            self.rebuild_key_index_from(name, relation)?;
        }
        Ok(())
    }
}
