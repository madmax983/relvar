//! Database Transaction Control (TCL) operations.

use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;

impl<E: StorageEngine> Database<E> {
    /// Begins a new transaction.
    ///
    /// This establishes a savepoint (snapshot) of the database. Any changes made
    /// subsequently are provisional until [`commit`](Self::commit) is called.
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
    /// Nested transactions are not currently supported.
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
        if self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "Transaction already in progress".to_string(),
            ));
        }

        let snapshot = self.engine.begin_transaction()?;
        self.transaction_snapshot = Some(snapshot);
        self.in_transaction = true;
        Ok(())
    }

    /// Commits the current transaction.
    ///
    /// Makes all changes since [`begin`](Self::begin) permanent and visible to others.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if no transaction is in progress.
    ///
    /// # Durability
    ///
    /// If using a persistent storage engine, this ensures all data and WAL entries
    /// are flushed to disk.
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

        if let Some(snapshot) = self.transaction_snapshot.take() {
            self.engine.commit_transaction(snapshot)?;
        }

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

        self.in_transaction = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DatabaseError;
    use crate::storage_engine::InMemoryEngine;

    #[test]
    fn test_transaction_rollback_when_no_transaction() {
        let mut db = Database::new(InMemoryEngine::new());
        let err = db.rollback().unwrap_err();
        assert!(matches!(err, DatabaseError::TransactionError(_)));
    }

    #[test]
    fn test_transaction_commit_when_no_transaction() {
        let mut db = Database::new(InMemoryEngine::new());
        let err = db.commit().unwrap_err();
        assert!(matches!(err, DatabaseError::TransactionError(_)));
    }

    #[test]
    fn test_transaction_begin_when_already_in_transaction() {
        let mut db = Database::new(InMemoryEngine::new());
        db.begin().unwrap();
        let err = db.begin().unwrap_err();
        assert!(matches!(err, DatabaseError::TransactionError(_)));
    }
}
