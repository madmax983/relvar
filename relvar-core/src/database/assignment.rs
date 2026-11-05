//! Multiple assignment (TTM RM Prescription 21).
//!
//! A multiple assignment assigns new values to several relvars as a single
//! statement. The Golden Rule (RM Prescription 23) applies at statement
//! boundaries only, so constraints spanning the assigned relvars are
//! checked on the combined final state — which is what makes it possible
//! to populate (or empty) relvars whose foreign keys reference each other.
//!
//! Several assignments may name the same relvar. TTM combines them: they
//! take effect in statement order, each on the result of the ones before
//! (`INSERT R r1, INSERT R r2` is `R := (R UNION r1) UNION r2`). Because
//! every source is a value computed before the statement starts, folding
//! the operations per target in order is exactly TTM's substitution
//! semantics. The fold is
//!
//! ```text
//! apply(r, Replace(v)) = v
//! apply(r, Insert(x))  = r ∪ x
//! apply(r, Delete(x))  = r − x
//! final(R) = fold_left(apply, R_before, ops on R in statement order)
//! ```

use crate::constraints::ConstraintManagerError;
use crate::database::Database;
use crate::database::golden_rule::ForeignKeyScope;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::Relation;
use alloc::string::String;
use alloc::vec::Vec;

/// One assignment of a [multiple assignment](Database::multiple_assign).
///
/// TTM: RM Prescription 21. `Insert` and `Delete` are the usual shorthands
/// for relational assignment (`INSERT R r` is `R := R UNION r`, `DELETE R r`
/// is `R := R MINUS r`). The relation's heading must be the target relvar's.
///
/// A [`Relation`] converts into [`Assignment::Replace`], so a plain
/// `(name, relation)` pair is a replacement.
///
/// # Examples
///
/// ```
/// use relvar_core::database::{Assignment, Database};
/// use relvar_core::storage_engine::InMemoryEngine;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::tuple;
///
/// let mut db = Database::new(InMemoryEngine::new());
/// let ty = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
/// db.create_relvar("R", ty.clone()).unwrap();
/// db.insert("R", tuple! { id: 1i64 }).unwrap();
///
/// let rel = |ids: &[i64]| Relation::from_tuples(ty.clone(), ids.iter().map(|i| tuple! { id: *i })).unwrap();
/// // INSERT R {2, 3}, DELETE R {1, 3}: R := ((R ∪ {2, 3}) − {1, 3}) = {2}.
/// db.multiple_assign(vec![
///     ("R", Assignment::Insert(rel(&[2, 3]))),
///     ("R", Assignment::Delete(rel(&[1, 3]))),
/// ])
/// .unwrap();
/// assert_eq!(db.query("R").unwrap(), rel(&[2]));
/// ```
#[derive(Debug, Clone)]
pub enum Assignment {
    /// `R := v`: the relvar's new value is `v`.
    Replace(Relation),
    /// `INSERT R x`: `R := R UNION x`.
    Insert(Relation),
    /// `DELETE R x`: `R := R MINUS x`.
    Delete(Relation),
}

impl Assignment {
    /// The relation this assignment's source is.
    pub fn relation(&self) -> &Relation {
        match self {
            Assignment::Replace(relation)
            | Assignment::Insert(relation)
            | Assignment::Delete(relation) => relation,
        }
    }

    /// Applies the assignment to `current`, the target's value so far.
    ///
    /// The headings were checked against the target before folding, so
    /// the set operators cannot fail on a type mismatch; if they did, that
    /// is reported as the heading mismatch it would be.
    fn apply(self, current: Relation) -> Result<Relation, DatabaseError> {
        match self {
            Assignment::Replace(value) => Ok(value),
            Assignment::Insert(inserted) => current
                .union_into(&inserted)
                .map_err(|_| ConstraintManagerError::TupleMismatch.into()),
            Assignment::Delete(deleted) => current
                .minus_into(&deleted)
                .map_err(|_| ConstraintManagerError::TupleMismatch.into()),
        }
    }
}

impl From<Relation> for Assignment {
    /// A bare relation is a replacement: `R := relation`.
    fn from(relation: Relation) -> Self {
        Assignment::Replace(relation)
    }
}

impl<E: StorageEngine> Database<E> {
    /// Assigns new values to several base relvars as one statement
    /// (multiple assignment).
    ///
    /// TTM: RM Prescription 21 — multiple assignment. Each `(relvar,
    /// assignment)` pair is an [`Assignment`]: a replacement (a bare
    /// [`Relation`] converts into one), an insert or a delete. A relvar may
    /// be named more than once; its assignments are combined in statement
    /// order, starting from its value before the statement
    /// (`Replace(v)` gives `v`, `Insert(x)` gives `current ∪ x`, `Delete(x)`
    /// gives `current − x`). Every source is a value computed before the
    /// statement, so this is TTM's semantics for repeated targets.
    ///
    /// The statement is atomic: every constraint is checked against the
    /// *final* state with all assignments applied, and either every
    /// assignment takes effect or none does. Intermediate states — within
    /// one target's operations or across targets — are never checked.
    /// Constraints checked:
    ///
    /// 1. each assignment's heading is the relvar's heading (checked for
    ///    every operation before anything else), and every tuple of each
    ///    final value conforms to it;
    /// 2. each relvar's keys, type constraints and CHECK constraints, on
    ///    its final value;
    /// 3. every foreign key out of or into an assigned relvar, evaluated on
    ///    the final value of each assigned side and the stored value of an
    ///    unassigned side;
    /// 4. every database assertion (after the writes, which are undone on
    ///    violation).
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
    /// # Performance
    ///
    /// A target whose first operation is `Insert` or `Delete` is loaded
    /// once; one whose first operation is `Replace` is not loaded at all.
    /// Each further operation costs one set operation on the value in hand.
    ///
    /// # Errors
    ///
    /// Nothing is changed when an error is returned.
    ///
    /// - [`DatabaseError::UnregisteredAssertion`] if a recorded closure
    ///   assertion has not been registered (see
    ///   [`unregistered_assertions`](Self::unregistered_assertions));
    /// - [`DatabaseError::CannotModifyVirtualRelvar`] if a target is a
    ///   virtual relvar;
    /// - [`DatabaseError::RelationNotFound`] if a target does not exist;
    /// - [`DatabaseError::Constraint`] wrapping
    ///   [`TupleMismatch`](crate::constraints::ConstraintManagerError::TupleMismatch)
    ///   (heading mismatch in any operation),
    ///   [`PrimaryKeyViolation`](crate::constraints::ConstraintManagerError::PrimaryKeyViolation),
    ///   [`CandidateKeyViolation`](crate::constraints::ConstraintManagerError::CandidateKeyViolation),
    ///   [`TypeConstraintViolation`](crate::constraints::ConstraintManagerError::TypeConstraintViolation),
    ///   [`CheckConstraintViolation`](crate::constraints::ConstraintManagerError::CheckConstraintViolation)
    ///   or [`ForeignKeyViolation`](crate::constraints::ConstraintManagerError::ForeignKeyViolation);
    /// - [`DatabaseError::AssertionViolation`] if a database assertion does
    ///   not hold on the final state, or [`DatabaseError::Assertion`] if a
    ///   declarative assertion cannot be evaluated;
    /// - [`DatabaseError::TransactionError`] from inside an assertion
    ///   predicate (predicates must be read-only);
    /// - a storage error if the engine cannot read or write a relvar.
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
    pub fn multiple_assign<N, A>(
        &mut self,
        assignments: impl IntoIterator<Item = (N, A)>,
    ) -> Result<(), DatabaseError>
    where
        N: Into<String>,
        A: Into<Assignment>,
    {
        let assignments: Vec<(String, Assignment)> = assignments
            .into_iter()
            .map(|(name, assignment)| (name.into(), assignment.into()))
            .collect();
        self.ensure_complete()?;
        if assignments.is_empty() {
            return Ok(());
        }

        // 1. Every target is an existing base relvar, and every
        //    operation's source has that relvar's heading. All of this is
        //    checked before any relvar is read.
        for (name, assignment) in &assignments {
            self.ensure_not_virtual(name)?;
            if !self.engine.relation_exists(name) {
                return Err(DatabaseError::RelationNotFound(name.clone()));
            }
            let heading = self.engine.get_relation_metadata(name)?.relation_type;
            if assignment.relation().relation_type() != &heading {
                return Err(ConstraintManagerError::TupleMismatch.into());
            }
        }

        // 2. Combine the operations per target, in statement order, from
        //    the target's pre-statement value.
        let finals = self.combine_assignments(assignments)?;

        // 3. Golden Rule on the final state: each final value against its
        //    relvar's own constraints, then every foreign key touching an
        //    assigned relvar, with all assignments applied at once.
        for (name, relation) in &finals {
            self.validate_relvar_value(name, relation, true)?;
        }
        let changes: Vec<(&str, &Relation)> = finals
            .iter()
            .map(|(name, relation)| (name.as_str(), relation))
            .collect();
        self.validate_foreign_keys_for_changes(&changes, ForeignKeyScope::All)?;

        // 4. Write every final value as one guarded statement: several
        //    writes need engine-level atomicity even without assertions.
        let names: Vec<&str> = finals.iter().map(|(name, _)| name.as_str()).collect();
        let guard = self.begin_statement_guard(&names, finals.len() > 1)?;
        for (name, relation) in &finals {
            if let Err(write_error) = self.engine.store_relation(name, relation) {
                self.abort_statement(guard)?;
                return Err(write_error.into());
            }
        }
        // Database assertions see the final state; a violation undoes
        // every write.
        self.finish_statement(guard)?;

        // 5. The statement took effect: refresh the key index (#23) from
        //    the values in hand.
        for (name, relation) in &finals {
            self.rebuild_key_index_from(name, relation)?;
        }
        Ok(())
    }

    /// Folds each target's assignments, in statement order, into its final
    /// value (see the module documentation). Targets keep the order of
    /// their first appearance.
    ///
    /// A target's stored value is loaded only if its first operation needs
    /// it (an `Insert` or a `Delete`).
    fn combine_assignments(
        &self,
        assignments: Vec<(String, Assignment)>,
    ) -> Result<Vec<(String, Relation)>, DatabaseError> {
        let mut finals: Vec<(String, Relation)> = Vec::new();
        for (name, assignment) in assignments {
            match finals.iter_mut().find(|(target, _)| *target == name) {
                Some((_, current)) => {
                    // Take the value out, leaving a cheap placeholder of the
                    // same type, so the fold can consume it.
                    let placeholder = Relation::new(current.relation_type().clone());
                    let value = core::mem::replace(current, placeholder);
                    *current = assignment.apply(value)?;
                }
                None => {
                    let value = match assignment {
                        Assignment::Replace(value) => value,
                        other => other.apply(self.engine.load_relation(&name)?)?,
                    };
                    finals.push((name, value));
                }
            }
        }
        Ok(finals)
    }
}
