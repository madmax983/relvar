//! Database Transaction Control (TCL) operations.

use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::{IsolationLevel, StorageEngine};

impl<E: StorageEngine> Database<E> {
    /// Begins a new transaction.
    ///
    /// This establishes a savepoint (snapshot) of the database. Any changes made
    /// subsequently are provisional until [`commit`](Self::commit) is called.
    ///
    /// The transaction runs at [`IsolationLevel::default`]
    /// ([`IsolationLevel::RepeatableRead`]); use
    /// [`begin_transaction_with_level`](Self::begin_transaction_with_level)
    /// to choose another level.
    ///
    /// # ACID Guarantees
    ///
    /// - **Isolation**: The transaction sees a consistent snapshot of the data.
    /// - **Atomicity**: Changes are not visible to other transactions until commit.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if a transaction is already active.
    /// Nested transactions are not currently supported.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin().unwrap();
    /// ```
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        self.begin_transaction_with_level(IsolationLevel::default())
    }

    /// Begins a new transaction at the given isolation level.
    ///
    /// The level governs which concurrent changes this transaction can
    /// observe while it runs; see [`IsolationLevel`] for the guarantees of
    /// each level. Engines without multi-version concurrency control (such
    /// as [`InMemoryEngine`](crate::storage_engine::InMemoryEngine)) accept
    /// any level but keep their existing snapshot semantics.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if a transaction is already active.
    /// Nested transactions are not currently supported.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::{InMemoryEngine, IsolationLevel};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin_transaction_with_level(IsolationLevel::ReadCommitted).unwrap();
    /// assert_eq!(db.isolation_level(), Some(IsolationLevel::ReadCommitted));
    /// ```
    pub fn begin_transaction_with_level(
        &mut self,
        level: IsolationLevel,
    ) -> Result<(), DatabaseError> {
        if self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "Transaction already in progress".to_string(),
            ));
        }

        let snapshot = self.engine.begin_transaction_with_isolation(level)?;
        self.transaction_snapshot = Some(snapshot);
        // Snapshot the key index too: rollback must restore it alongside
        // the engine state (#23).
        self.key_index_snapshot = Some(self.key_index.clone());
        self.transaction_isolation = Some(level);
        self.in_transaction = true;
        Ok(())
    }

    /// Returns the isolation level of the active transaction, if any.
    ///
    /// Returns `None` when no transaction is in progress.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::{InMemoryEngine, IsolationLevel};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// assert_eq!(db.isolation_level(), None);
    /// db.begin().unwrap();
    /// assert_eq!(db.isolation_level(), Some(IsolationLevel::RepeatableRead));
    /// ```
    pub fn isolation_level(&self) -> Option<IsolationLevel> {
        self.transaction_isolation
    }

    /// Commits the current transaction.
    ///
    /// Makes all changes since [`begin`](Self::begin) permanent and visible to others.
    ///
    /// A [`IsolationLevel::Serializable`] transaction whose commit validation
    /// fails (a concurrent transaction committed overlapping changes) is
    /// aborted instead: its changes are discarded and the error is returned.
    /// Either way the database is left with no transaction in progress.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if no transaction is in progress.
    /// Returns the engine's error (e.g. a serialization failure) if the
    /// commit itself fails.
    ///
    /// # Durability
    ///
    /// If using a persistent storage engine, this ensures all data and WAL entries
    /// are flushed to disk.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin().unwrap();
    /// db.commit().unwrap();
    /// ```
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        if !self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "No transaction in progress".to_string(),
            ));
        }

        if let Some(snapshot) = self.transaction_snapshot.take()
            && let Err(engine_error) = self.engine.commit_transaction(snapshot)
        {
            // Contract (`StorageEngine::commit_transaction`): a failed
            // commit ends the engine-side transaction, so unwind the
            // database-side transaction state too. This covers e.g. a
            // serializable validation failure, where the engine aborted
            // the transaction instead of committing it.
            if let Some(index_snapshot) = self.key_index_snapshot.take() {
                self.key_index = index_snapshot;
            }
            self.transaction_isolation = None;
            self.in_transaction = false;
            return Err(engine_error.into());
        }

        // The index already reflects the committed state; drop the snapshot.
        self.key_index_snapshot = None;

        self.transaction_isolation = None;
        self.in_transaction = false;
        Ok(())
    }

    /// Rolls back the current transaction.
    ///
    /// Discards all changes made since [`begin`](Self::begin), restoring the database
    /// to its state at the start of the transaction.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if no transaction is in progress.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin().unwrap();
    /// db.rollback().unwrap();
    /// ```
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        if !self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "No transaction in progress".to_string(),
            ));
        }

        if let Some(snapshot) = self.transaction_snapshot.take() {
            self.engine.rollback_transaction(snapshot)?;
        }

        // Restore the key index to its pre-transaction state (#23).
        if let Some(index_snapshot) = self.key_index_snapshot.take() {
            self.key_index = index_snapshot;
        }

        self.transaction_isolation = None;
        self.in_transaction = false;
        Ok(())
    }
}
