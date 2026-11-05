//! Statement atomicity and database-assertion enforcement.
//!
//! TTM RM Prescription 23 (the Golden Rule) requires every constraint to
//! hold at every statement boundary, and a statement either takes effect as
//! a whole or not at all. Most constraints are validated against the
//! candidate post-statement state *before* anything is written. Database
//! assertions are arbitrary predicates over the database, so they are
//! evaluated *after* the statement's writes; a [`StatementGuard`] records
//! how to undo those writes if an assertion fails.
//!
//! How the undo works depends on whether a user transaction is active:
//!
//! - **No user transaction**: the statement runs in its own engine
//!   transaction, committed or rolled back as a unit.
//! - **Inside a user transaction**: opening a second engine transaction is
//!   not an option — on a multi-version engine such as `PersistentEngine`
//!   it would displace the user's ambient transaction, and its commit would
//!   end the user's transaction context, so later statements would
//!   autocommit and the user's rollback would undo nothing. Instead the
//!   guard keeps the pre-statement values of the relvars the statement
//!   writes and restores them (within the user's transaction) on failure.

use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::Relation;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// How to undo a statement's writes if it fails after writing.
pub(crate) enum StatementGuard<S> {
    /// Nothing to undo with: no assertions are registered and the statement
    /// does not need engine-level atomicity (it performs a single write).
    Unguarded,
    /// The statement runs in its own engine transaction (no user
    /// transaction was active).
    Engine(S),
    /// Inside a user transaction: the pre-statement values of the relvars
    /// the statement writes.
    Restore(Vec<(String, Relation)>),
}

impl<E: StorageEngine> Database<E> {
    /// Begins guarding a statement that writes the relvars in `affected`.
    ///
    /// `atomic` requests engine-level atomicity even when no assertions are
    /// registered (needed by statements that perform several writes).
    ///
    /// # Errors
    ///
    /// [`DatabaseError::TransactionError`] when called from inside an
    /// assertion predicate (predicates must be read-only), or a storage
    /// error if the engine cannot begin a transaction or load a relvar.
    pub(crate) fn begin_statement_guard(
        &mut self,
        affected: &[&str],
        atomic: bool,
    ) -> Result<StatementGuard<E::Snapshot>, DatabaseError> {
        if self.evaluating_assertions {
            return Err(DatabaseError::TransactionError(
                "assertion predicates must not perform DML".to_string(),
            ));
        }
        if self.assertions.is_empty() && !atomic {
            return Ok(StatementGuard::Unguarded);
        }
        if self.in_transaction {
            let mut saved = Vec::with_capacity(affected.len());
            for name in affected {
                saved.push(((*name).to_string(), self.engine.load_relation(name)?));
            }
            return Ok(StatementGuard::Restore(saved));
        }
        Ok(StatementGuard::Engine(self.engine.begin_transaction()?))
    }

    /// Undoes a guarded statement's writes.
    ///
    /// # Errors
    ///
    /// A storage error if the engine cannot roll back or restore a relvar.
    pub(crate) fn abort_statement(
        &mut self,
        guard: StatementGuard<E::Snapshot>,
    ) -> Result<(), DatabaseError> {
        match guard {
            StatementGuard::Unguarded => Ok(()),
            StatementGuard::Engine(snapshot) => Ok(self.engine.rollback_transaction(snapshot)?),
            StatementGuard::Restore(saved) => {
                for (name, relation) in &saved {
                    self.engine.store_relation(name, relation)?;
                }
                Ok(())
            }
        }
    }

    /// Completes a guarded statement whose writes succeeded: checks every
    /// database assertion against the post-statement state, undoing the
    /// statement on violation, and otherwise commits its engine
    /// transaction (if it has one).
    ///
    /// # Errors
    ///
    /// [`DatabaseError::AssertionViolation`] naming the first violated
    /// assertion (the statement has been undone), or a storage error if the
    /// engine cannot roll back, restore or commit.
    pub(crate) fn finish_statement(
        &mut self,
        guard: StatementGuard<E::Snapshot>,
    ) -> Result<(), DatabaseError> {
        if let Err(violation) = self.evaluate_assertions() {
            self.abort_statement(guard)?;
            return Err(violation.into());
        }
        if let StatementGuard::Engine(snapshot) = guard {
            self.engine.commit_transaction(snapshot)?;
        }
        Ok(())
    }
}
