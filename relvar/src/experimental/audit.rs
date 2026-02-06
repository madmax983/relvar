//! Audited Database Wrapper.
//!
//! This module provides a wrapper around [`Database`] that automatically logs
//! all data modifications (INSERT, UPDATE, DELETE) to a system audit log relvar.
//!
//! # The "_AUDIT_LOG" Relvar
//!
//! All changes are recorded in a relvar named `_AUDIT_LOG` with the following schema:
//!
//! - `id`: Int (Timestamp + Sequence) - Primary Key
//! - `timestamp`: Int (Unix timestamp in milliseconds)
//! - `operation`: String ("INSERT", "UPDATE", "DELETE")
//! - `relvar_name`: String (Name of the modified relvar)
//! - `before_image`: String (JSON representation of the tuple before modification, empty for INSERT)
//! - `after_image`: String (JSON representation of the tuple after modification, empty for DELETE)
//!
//! # Example
//!
//! ```
//! use relvar::experimental::audit::AuditedDatabase;
//! use relvar::InMemoryEngine;
//! use relvar_core::database::Database;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//!
//! let db = Database::new(InMemoryEngine::new());
//! let mut audited_db = AuditedDatabase::new(db).unwrap();
//!
//! let rel_type = RelationType::new(
//!     TupleType::new().with_attribute("id", ScalarType::Int)
//! );
//! audited_db.create_relvar("TEST", rel_type).unwrap();
//!
//! // This insert is automatically logged!
//! audited_db.insert("TEST", tuple! { id: 1i64 }).unwrap();
//!
//! // Verify log
//! let logs = audited_db.query("_AUDIT_LOG").unwrap();
//! assert_eq!(logs.cardinality(), 1);
//! ```

use relvar_core::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints, KeyConstraints, PrimaryKey,
};
use relvar_core::database::{Database, DatabaseError};
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
#[cfg(test)]
use relvar_core::values::ScalarValue;
use relvar_core::values::{Relation, Tuple};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIT_LOG_NAME: &str = "_AUDIT_LOG";

/// A wrapper around `Database` that provides automatic audit logging.
pub struct AuditedDatabase<E: StorageEngine> {
    inner: Database<E>,
    sequence: AtomicU64,
}

impl<E: StorageEngine> AuditedDatabase<E> {
    /// Create a new audited database wrapper.
    ///
    /// This will automatically create the `_AUDIT_LOG` relvar if it doesn't exist.
    pub fn new(mut inner: Database<E>) -> Result<Self, DatabaseError> {
        if !inner.relvar_exists(AUDIT_LOG_NAME) {
            let log_type = RelationType::new(
                TupleType::new()
                    .with_attribute("id", ScalarType::Int)
                    .with_attribute("timestamp", ScalarType::Int)
                    .with_attribute("operation", ScalarType::String)
                    .with_attribute("relvar_name", ScalarType::String)
                    .with_attribute("before_image", ScalarType::String)
                    .with_attribute("after_image", ScalarType::String),
            );

            inner.create_relvar(AUDIT_LOG_NAME, log_type)?;

            // Set Primary Key on ID
            let pk = PrimaryKey::new(vec!["id".to_string()]).map_err(|e| {
                DatabaseError::Constraint(
                    relvar_core::constraints::ConstraintManagerError::TransactionError(
                        e.to_string(),
                    ),
                )
            })?;
            let constraints = KeyConstraints::new().with_primary_key(pk);
            inner.set_key_constraints(AUDIT_LOG_NAME, constraints)?;
        }

        Ok(Self {
            inner,
            sequence: AtomicU64::new(0),
        })
    }

    /// Access the inner database.
    pub fn inner(&self) -> &Database<E> {
        &self.inner
    }

    /// Access the inner database mutably.
    ///
    /// **Warning:** modifying the database directly bypasses the audit log.
    pub fn inner_mut(&mut self) -> &mut Database<E> {
        &mut self.inner
    }

    // --- Intercepted Methods ---

    /// Insert a tuple and log the operation.
    pub fn insert(&mut self, relvar_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        // Prevent infinite recursion by not logging inserts to the audit log itself
        if relvar_name != AUDIT_LOG_NAME {
            let after_json = serde_json::to_string(&tuple).unwrap_or_default();
            self.log_change(relvar_name, "INSERT", "", &after_json)?;
        }

        self.inner.insert(relvar_name, tuple)
    }

    /// Delete tuples and log the operation.
    ///
    /// Requires the predicate to be `Clone` to allow pre-execution query.
    pub fn delete<F>(&mut self, relvar_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool + Clone,
    {
        if relvar_name != AUDIT_LOG_NAME {
            // Query tuples to be deleted
            let relation = self.inner.query(relvar_name)?;
            for tuple in relation.tuples() {
                if predicate(tuple) {
                    let before_json = serde_json::to_string(tuple).unwrap_or_default();
                    self.log_change(relvar_name, "DELETE", &before_json, "")?;
                }
            }
        }

        self.inner.delete(relvar_name, predicate)
    }

    /// Update tuples and log the operation.
    ///
    /// Requires the predicate and updater to be `Clone`.
    pub fn update<F, U>(
        &mut self,
        relvar_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool + Clone,
        U: Fn(&Tuple) -> Tuple + Clone,
    {
        if relvar_name != AUDIT_LOG_NAME {
            // Query tuples to be updated
            let relation = self.inner.query(relvar_name)?;
            for tuple in relation.tuples() {
                if predicate(tuple) {
                    let before_json = serde_json::to_string(tuple).unwrap_or_default();
                    let new_tuple = updater(tuple);
                    let after_json = serde_json::to_string(&new_tuple).unwrap_or_default();

                    self.log_change(relvar_name, "UPDATE", &before_json, &after_json)?;
                }
            }
        }

        self.inner.update(relvar_name, predicate, updater)
    }

    // --- Helper for Logging ---

    fn log_change(
        &mut self,
        relvar_name: &str,
        operation: &str,
        before: &str,
        after: &str,
    ) -> Result<(), DatabaseError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) as i64;
        // Simple composite ID: timestamp << 20 | seq (to fit in i64 somewhat uniquely)
        // Or just use timestamp if granularity is low.
        // Let's use a simpler approach: just timestamp for now, but assume single threaded for tests.
        // Actually, to avoid PK violation on fast inserts, we need a unique ID.
        // Let's use nanoseconds if possible, or just a counter.
        // Since we are in `new`, we reset sequence.
        // Let's rely on timestamp + seq.
        // But `id` is a single Int.
        // Let's construct a synthetic ID.
        let id = (timestamp * 1000) + (seq % 1000);

        let log_tuple = tuple! {
            id: id,
            timestamp: timestamp,
            operation: operation,
            relvar_name: relvar_name,
            before_image: before,
            after_image: after
        };

        self.inner.insert(AUDIT_LOG_NAME, log_tuple)
    }

    // --- Forwarded Methods ---

    /// Create a relvar.
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        self.inner.create_relvar(name, relation_type)
    }

    /// Drop a relvar.
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.inner.drop_relvar(name)
    }

    /// Check if relvar exists.
    pub fn relvar_exists(&self, name: &str) -> bool {
        self.inner.relvar_exists(name)
    }

    /// List relvars.
    pub fn list_relvars(&self) -> Vec<String> {
        self.inner.list_relvars()
    }

    /// Get relvar type.
    pub fn get_relvar_type(&self, name: &str) -> Result<RelationType, DatabaseError> {
        self.inner.get_relvar_type(name)
    }

    /// Query a relvar.
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        self.inner.query(relation_name)
    }

    /// Begin transaction.
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        self.inner.begin()
    }

    /// Commit transaction.
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        self.inner.commit()
    }

    /// Rollback transaction.
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        self.inner.rollback()
    }

    /// Define virtual relvar.
    pub fn define_virtual_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
        evaluator: fn(&mut Database<E>) -> Result<Relation, DatabaseError>,
    ) -> Result<(), DatabaseError> {
        self.inner
            .define_virtual_relvar(name, relation_type, evaluator)
    }

    /// Drop virtual relvar.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.inner.drop_virtual_relvar(name)
    }

    /// Set key constraints.
    pub fn set_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), DatabaseError> {
        self.inner.set_key_constraints(relation_name, constraints)
    }

    /// Get key constraints.
    pub fn get_key_constraints(&self, relation_name: &str) -> Option<&KeyConstraints> {
        self.inner.get_key_constraints(relation_name)
    }

    /// Set foreign key constraints.
    pub fn set_foreign_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: ForeignKeyConstraints,
    ) -> Result<(), DatabaseError> {
        self.inner
            .set_foreign_key_constraints(relation_name, constraints)
    }

    /// Get foreign key constraints.
    pub fn get_foreign_key_constraints(
        &self,
        relation_name: &str,
    ) -> Option<&ForeignKeyConstraints> {
        self.inner.get_foreign_key_constraints(relation_name)
    }

    /// Set type constraints.
    pub fn set_type_constraints(
        &mut self,
        relation_name: &str,
        attribute_name: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), DatabaseError> {
        self.inner
            .set_type_constraints(relation_name, attribute_name, constraints)
    }

    /// Set check constraints.
    pub fn set_check_constraints(
        &mut self,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), DatabaseError> {
        self.inner.set_check_constraints(relation_name, constraints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::storage_engine::InMemoryEngine;

    #[test]
    fn test_audit_log_created_on_new() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        assert!(aud_db.relvar_exists(AUDIT_LOG_NAME));

        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 0);
    }

    #[test]
    fn test_insert_logged() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();

        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();

        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 1);

        let log = logs.tuples().next().unwrap();
        assert_eq!(log.get_typed::<String>("operation").unwrap(), "INSERT");
        assert_eq!(log.get_typed::<String>("relvar_name").unwrap(), "TEST");
        assert!(
            log.get_typed::<String>("after_image")
                .unwrap()
                .contains("1")
        );
    }

    #[test]
    fn test_delete_logged() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();
        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();

        // Delete
        aud_db
            .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
            .unwrap();

        // Should have INSERT and DELETE logs
        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 2);

        // Find DELETE log
        let mut found_delete = false;
        for log in logs.tuples() {
            if log.get_typed::<String>("operation").unwrap() == "DELETE" {
                found_delete = true;
                assert!(
                    log.get_typed::<String>("before_image")
                        .unwrap()
                        .contains("1")
                );
            }
        }
        assert!(found_delete);
    }

    #[test]
    fn test_update_logged() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();
        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();

        // Update
        aud_db
            .update(
                "TEST",
                |t| t.get_typed::<i64>("id").unwrap() == 1,
                |_| tuple! { id: 2i64 },
            )
            .unwrap();

        // Should have INSERT and UPDATE logs
        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 2);

        // Find UPDATE log
        let mut found_update = false;
        for log in logs.tuples() {
            if log.get_typed::<String>("operation").unwrap() == "UPDATE" {
                found_update = true;
                assert!(
                    log.get_typed::<String>("before_image")
                        .unwrap()
                        .contains("1")
                );
                assert!(
                    log.get_typed::<String>("after_image")
                        .unwrap()
                        .contains("2")
                );
            }
        }
        assert!(found_update);
    }

    #[test]
    fn test_recursion_guard() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        // Manually insert into _AUDIT_LOG
        // This should NOT trigger another log entry (which would cause infinite recursion)
        let manual_log = tuple! {
            id: 9999i64,
            timestamp: 12345i64,
            operation: "MANUAL",
            relvar_name: "TEST",
            before_image: "",
            after_image: ""
        };

        aud_db.insert(AUDIT_LOG_NAME, manual_log).unwrap();

        // There should be exactly 1 log entry (the one we just inserted)
        // If recursion wasn't guarded, we'd have 2 or more (stack overflow)
        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 1);
    }

    #[test]
    fn test_forwarding_accessors() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();

        // inner()
        assert!(aud_db.inner().relvar_exists("TEST"));

        // inner_mut()
        aud_db.inner_mut().insert("TEST", tuple!{ id: 100i64 }).unwrap();
        assert_eq!(aud_db.query("TEST").unwrap().cardinality(), 1);

        // Mutation via inner_mut should NOT be logged
        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 0);
    }

    #[test]
    fn test_forwarding_schema_ops() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        // create_relvar
        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();

        // relvar_exists
        assert!(aud_db.relvar_exists("TEST"));

        // list_relvars
        let relvars = aud_db.list_relvars();
        assert!(relvars.contains(&"TEST".to_string()));
        assert!(relvars.contains(&AUDIT_LOG_NAME.to_string()));

        // get_relvar_type
        let type_info = aud_db.get_relvar_type("TEST").unwrap();
        assert!(type_info.heading().has_attribute("id"));

        // drop_relvar
        aud_db.drop_relvar("TEST").unwrap();
        assert!(!aud_db.relvar_exists("TEST"));
    }

    #[test]
    fn test_forwarding_transactions() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();

        // Begin
        aud_db.begin().unwrap();
        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();

        // Should see uncommitted change
        assert_eq!(aud_db.query("TEST").unwrap().cardinality(), 1);

        // Rollback
        aud_db.rollback().unwrap();
        assert_eq!(aud_db.query("TEST").unwrap().cardinality(), 0);

        // Commit flow
        aud_db.begin().unwrap();
        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();
        aud_db.commit().unwrap();
        assert_eq!(aud_db.query("TEST").unwrap().cardinality(), 1);

        // Verify logs were also committed
        let logs = aud_db.query(AUDIT_LOG_NAME).unwrap();
        assert_eq!(logs.cardinality(), 1);
    }

    #[test]
    fn test_forwarding_virtual_relvars() {
        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type.clone()).unwrap();
        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();

        // define_virtual_relvar
        aud_db.define_virtual_relvar(
            "V_TEST",
            rel_type,
            |db| db.query("TEST")
        ).unwrap();

        assert!(aud_db.relvar_exists("V_TEST"));

        let v_result = aud_db.query("V_TEST").unwrap();
        assert_eq!(v_result.cardinality(), 1);

        // drop_virtual_relvar
        aud_db.drop_virtual_relvar("V_TEST").unwrap();
        assert!(!aud_db.relvar_exists("V_TEST"));
    }

    #[test]
    fn test_forwarding_constraints() {
        use relvar_core::constraints::{CheckConstraint, ConstraintExpression, ValueOrRef};

        let db = Database::new(InMemoryEngine::new());
        let mut aud_db = AuditedDatabase::new(db).unwrap();

        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        aud_db.create_relvar("TEST", rel_type).unwrap();

        // Set PK
        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let key_constraints = KeyConstraints::new().with_primary_key(pk);
        aud_db.set_key_constraints("TEST", key_constraints).unwrap();

        // Get PK
        assert!(aud_db.get_key_constraints("TEST").is_some());

        // Set Check Constraint
        let check = CheckConstraints::new().with_constraint(
            CheckConstraint::from_expression(
                "positive_id", "ID > 0",
                ConstraintExpression::Gt(
                    "id".to_string(),
                    ValueOrRef::Value(ScalarValue::Int(0))
                )
            )
        );
        aud_db.set_check_constraints("TEST", check).unwrap();

        // Verify constraint logic via insert forwarding
        let result = aud_db.insert("TEST", tuple! { id: -1i64 });
        assert!(result.is_err()); // Check constraint violation

        // Verify PK logic
        aud_db.insert("TEST", tuple! { id: 1i64 }).unwrap();
        let result = aud_db.insert("TEST", tuple! { id: 1i64 });
        assert!(result.is_err()); // PK violation
    }
}
