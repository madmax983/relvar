//! Audit logging wrapper for Database.
//!
//! This module provides `AuditedDatabase`, a wrapper around `Database` that automatically
//! logs insert, update, and delete operations to a system relvar `_AUDIT_LOG`.

use relvar_core::database::{Database, DatabaseError};
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, Tuple};
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIT_LOG_RELVAR: &str = "_AUDIT_LOG";

/// A wrapper around Database that logs operations.
pub struct AuditedDatabase<E: StorageEngine> {
    inner: Database<E>,
    audit_enabled: bool,
}

impl<E: StorageEngine> AuditedDatabase<E> {
    /// Create a new AuditedDatabase.
    pub fn new(inner: Database<E>) -> Self {
        Self {
            inner,
            audit_enabled: true,
        }
    }

    /// Initialize the audit log relvar if it doesn't exist.
    pub fn init(&mut self) -> Result<(), DatabaseError> {
        if !self.inner.relvar_exists(AUDIT_LOG_RELVAR) {
            let rel_type = RelationType::new(
                TupleType::new()
                    .with_attribute("id", ScalarType::Int)
                    .with_attribute("timestamp", ScalarType::Int)
                    .with_attribute("action", ScalarType::String)
                    .with_attribute("target", ScalarType::String)
                    .with_attribute("content", ScalarType::String),
            );
            self.inner.create_relvar(AUDIT_LOG_RELVAR, rel_type)?;
        }
        Ok(())
    }

    /// Insert a tuple and log it.
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        self.inner.insert(relation_name, tuple.clone())?;

        if self.should_log(relation_name) {
            self.log_action("INSERT", relation_name, &tuple)?;
        }

        Ok(())
    }

    /// Delete tuples matching a predicate and log the deleted tuples.
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool + Clone,
    {
        let to_delete = if self.should_log(relation_name) {
            // Identify what will be deleted
            // We allow query failure (e.g. if relvar doesn't exist, inner.delete will catch it)
            if let Ok(relation) = self.inner.query(relation_name) {
                Some(
                    relation
                        .tuples()
                        .filter(|t| predicate(t))
                        .cloned()
                        .collect::<Vec<Tuple>>(),
                )
            } else {
                None
            }
        } else {
            None
        };

        let count = self.inner.delete(relation_name, predicate)?;

        if let Some(deleted_tuples) = to_delete {
            for t in &deleted_tuples {
                self.log_action("DELETE", relation_name, t)?;
            }
        }

        Ok(count)
    }

    /// Update tuples matching a predicate and log the *old* values.
    #[allow(clippy::collapsible_if)]
    pub fn update<F, U>(
        &mut self,
        relation_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool + Clone,
        U: Fn(&Tuple) -> Tuple,
    {
        let to_update = if self.should_log(relation_name) {
            if let Ok(relation) = self.inner.query(relation_name) {
                Some(
                    relation
                        .tuples()
                        .filter(|t| predicate(t))
                        .cloned()
                        .collect::<Vec<Tuple>>(),
                )
            } else {
                None
            }
        } else {
            None
        };

        let count = self.inner.update(relation_name, predicate, updater)?;

        if let Some(updated_tuples) = to_update {
            for t in &updated_tuples {
                self.log_action("UPDATE_OLD", relation_name, t)?;
            }
        }

        Ok(count)
    }

    /// Query a relation.
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        self.inner.query(relation_name)
    }

    /// Create a relvar.
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        self.inner.create_relvar(name, relation_type)
    }

    /// Begin a transaction.
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        self.inner.begin()
    }

    /// Commit a transaction.
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        self.inner.commit()
    }

    /// Rollback a transaction.
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        self.inner.rollback()
    }

    fn should_log(&self, relation_name: &str) -> bool {
        self.audit_enabled && relation_name != AUDIT_LOG_RELVAR
    }

    fn log_action(
        &mut self,
        action: &str,
        target: &str,
        content: &Tuple,
    ) -> Result<(), DatabaseError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let next_id = self.get_next_id()?;

        let content_json = serde_json::to_string(&content).unwrap_or_else(|_| "{}".to_string());

        let log_entry = tuple! {
            id: next_id,
            timestamp: timestamp,
            action: action,
            target: target,
            content: content_json
        };

        // We must recursively call inner insert to avoid infinite recursion of logging!
        self.inner.insert(AUDIT_LOG_RELVAR, log_entry)?;
        Ok(())
    }

    fn get_next_id(&mut self) -> Result<i64, DatabaseError> {
        let relation = self.inner.query(AUDIT_LOG_RELVAR)?;
        if relation.cardinality() == 0 {
            return Ok(1);
        }

        let max_id = relation
            .tuples()
            .map(|t| t.get_typed::<i64>("id").unwrap_or(0))
            .max()
            .unwrap_or(0);

        Ok(max_id + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::TupleType;

    fn setup() -> AuditedDatabase<InMemoryEngine> {
        let db = Database::new(InMemoryEngine::new());
        let mut audited = AuditedDatabase::new(db);
        audited.init().unwrap();
        audited
    }

    #[test]
    fn test_init_creates_audit_log() {
        let mut db = setup();
        assert!(db.query(AUDIT_LOG_RELVAR).is_ok());
    }

    #[test]
    fn test_insert_logs_action() {
        let mut db = setup();

        let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::Int));
        db.create_relvar("TEST", rel_type).unwrap();

        db.insert("TEST", tuple! { val: 42i64 }).unwrap();

        let logs = db.query(AUDIT_LOG_RELVAR).unwrap();
        assert_eq!(logs.cardinality(), 1);

        let entry = logs.tuples().next().unwrap();
        assert_eq!(entry.get_typed::<String>("action").unwrap(), "INSERT");
        assert_eq!(entry.get_typed::<String>("target").unwrap(), "TEST");
        assert!(entry.get_typed::<String>("content").unwrap().contains("42"));
    }

    #[test]
    fn test_delete_logs_action() {
        let mut db = setup();

        let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::Int));
        db.create_relvar("TEST", rel_type).unwrap();
        db.insert("TEST", tuple! { val: 42i64 }).unwrap();

        // Delete
        db.delete("TEST", |t| t.get_typed::<i64>("val").unwrap() == 42)
            .unwrap();

        let logs = db.query(AUDIT_LOG_RELVAR).unwrap();
        // Should have INSERT and DELETE
        assert_eq!(logs.cardinality(), 2);

        let mut entries: Vec<_> = logs.tuples().collect();
        // Sort by id
        entries.sort_by_key(|t| t.get_typed::<i64>("id").unwrap());

        let delete_entry = &entries[1];
        assert_eq!(
            delete_entry.get_typed::<String>("action").unwrap(),
            "DELETE"
        );
        assert_eq!(delete_entry.get_typed::<String>("target").unwrap(), "TEST");
    }

    #[test]
    fn test_update_logs_old_value() {
        let mut db = setup();

        let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::Int));
        db.create_relvar("TEST", rel_type).unwrap();
        db.insert("TEST", tuple! { val: 42i64 }).unwrap();

        // Update
        db.update(
            "TEST",
            |t| t.get_typed::<i64>("val").unwrap() == 42,
            |_| tuple! { val: 100i64 },
        )
        .unwrap();

        let logs = db.query(AUDIT_LOG_RELVAR).unwrap();
        // Should have INSERT and UPDATE_OLD
        assert_eq!(logs.cardinality(), 2);

        let mut entries: Vec<_> = logs.tuples().collect();
        entries.sort_by_key(|t| t.get_typed::<i64>("id").unwrap());

        let update_entry = &entries[1];
        assert_eq!(
            update_entry.get_typed::<String>("action").unwrap(),
            "UPDATE_OLD"
        );
        assert!(
            update_entry
                .get_typed::<String>("content")
                .unwrap()
                .contains("42")
        );
    }
}
