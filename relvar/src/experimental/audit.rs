//! Audit logging wrapper for Database.
//!
//! This module provides `AuditedDatabase`, a wrapper around `Database` that
//! automatically logs all modification operations to a system relvar named `_AUDIT_LOG`.

use relvar_core::database::{Database, DatabaseError};
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, Tuple};
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIT_RELVAR: &str = "_AUDIT_LOG";

/// A wrapper around `Database` that logs operations.
pub struct AuditedDatabase<E: StorageEngine> {
    db: Database<E>,
}

impl<E: StorageEngine> AuditedDatabase<E> {
    /// Create a new audited database.
    ///
    /// This will automatically create the `_AUDIT_LOG` relvar if it does not exist.
    pub fn new(mut db: Database<E>) -> Result<Self, DatabaseError> {
        if !db.relvar_exists(AUDIT_RELVAR) {
            let audit_type = RelationType::new(
                TupleType::new()
                    .with_attribute("timestamp", ScalarType::Int)
                    .with_attribute("operation", ScalarType::String)
                    .with_attribute("target", ScalarType::String)
                    .with_attribute("details", ScalarType::String),
            );
            db.create_relvar(AUDIT_RELVAR, audit_type)?;
        }
        Ok(Self { db })
    }

    /// Returns a mutable reference to the inner database.
    pub fn inner(&mut self) -> &mut Database<E> {
        &mut self.db
    }

    /// Insert a tuple and log the operation.
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        // Skip auditing the audit log itself to prevent infinite recursion
        if relation_name == AUDIT_RELVAR {
            return self.db.insert(relation_name, tuple);
        }

        // Perform the insert
        self.db.insert(relation_name, tuple.clone())?;

        // Log the operation
        let details = serde_json::to_string(&tuple).map_err(|e| {
            DatabaseError::TransactionError(format!("Audit serialization error: {}", e))
        })?;

        self.log_operation("INSERT", relation_name, details)?;
        Ok(())
    }

    /// Delete tuples and log the operation.
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        if relation_name == AUDIT_RELVAR {
            return self.db.delete(relation_name, predicate);
        }

        let count = self.db.delete(relation_name, predicate)?;

        if count > 0 {
            self.log_operation("DELETE", relation_name, format!("Deleted {} rows", count))?;
        }
        Ok(count)
    }

    /// Update tuples and log the operation.
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
        if relation_name == AUDIT_RELVAR {
            return self.db.update(relation_name, predicate, updater);
        }

        let count = self.db.update(relation_name, predicate, updater)?;

        if count > 0 {
            self.log_operation("UPDATE", relation_name, format!("Updated {} rows", count))?;
        }
        Ok(count)
    }

    /// Query the database.
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        self.db.query(relation_name)
    }

    /// Create a new relvar.
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        self.db.create_relvar(name, relation_type)
    }

    /// Drop a relvar.
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.db.drop_relvar(name)
    }

    /// Begin a transaction.
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        self.db.begin()
    }

    /// Commit a transaction.
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        self.db.commit()
    }

    /// Rollback a transaction.
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        self.db.rollback()
    }

    fn log_operation(
        &mut self,
        op: &str,
        target: &str,
        details: String,
    ) -> Result<(), DatabaseError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let log_entry = tuple! {
            timestamp: timestamp,
            operation: op,
            target: target,
            details: details
        };

        // We use inner db insert to avoid recursion check in public insert
        self.db.insert(AUDIT_RELVAR, log_entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_audit_log_creation() {
        let db = Database::new(InMemoryEngine::new());
        let mut audited_db = AuditedDatabase::new(db).unwrap();

        assert!(audited_db.inner().relvar_exists(AUDIT_RELVAR));
    }

    #[test]
    fn test_audit_log_insert() {
        let db = Database::new(InMemoryEngine::new());
        let mut audited_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        );
        audited_db.create_relvar("TEST", rel_type).unwrap();

        audited_db
            .insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Verify data inserted
        let result = audited_db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);

        // Verify audit log
        let audit_log = audited_db.query(AUDIT_RELVAR).unwrap();
        assert_eq!(audit_log.cardinality(), 1);

        let entry = audit_log.tuples().next().unwrap();
        assert_eq!(entry.get_typed::<String>("operation").unwrap(), "INSERT");
        assert_eq!(entry.get_typed::<String>("target").unwrap(), "TEST");
        assert!(
            entry
                .get_typed::<String>("details")
                .unwrap()
                .contains("Alice")
        );
    }

    #[test]
    fn test_audit_log_delete() {
        let db = Database::new(InMemoryEngine::new());
        let mut audited_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        audited_db.create_relvar("TEST", rel_type).unwrap();
        audited_db.insert("TEST", tuple! { id: 1i64 }).unwrap();

        audited_db.delete("TEST", |_| true).unwrap();

        let audit_log = audited_db.query(AUDIT_RELVAR).unwrap();
        // Should have insert log and delete log
        assert_eq!(audit_log.cardinality(), 2);
    }
}
