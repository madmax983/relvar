use rand::Rng;
use relvar_core::constraints::{ConstraintManagerError, KeyConstraints, PrimaryKey};
use relvar_core::database::{Database, DatabaseError};
use relvar_core::storage_engine::StorageEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, Tuple};
use std::time::{SystemTime, UNIX_EPOCH};

/// A wrapper around `Database` that automatically logs all data modifications.
///
/// This experimental feature creates an `_AUDIT_LOG` system relvar that tracks:
/// - Operation type (INSERT, UPDATE, DELETE)
/// - Timestamp
/// - Affected relation
/// - Before/After images of the tuple (as JSON)
///
/// # Example
///
/// ```
/// use relvar::{Database, InMemoryEngine, tuple};
/// use relvar::types::{TupleType, RelationType, ScalarType};
/// use relvar::experimental::audit::AuditedDatabase;
///
/// let db = Database::new(InMemoryEngine::new());
/// let mut audited_db = AuditedDatabase::new(db).unwrap();
///
/// let rel_type = RelationType::new(
///     TupleType::new().with_attribute("id", ScalarType::Int)
/// );
/// audited_db.create_relvar("TEST", rel_type).unwrap();
///
/// // This insert will be audited
/// audited_db.insert("TEST", tuple! { id: 1i64 }).unwrap();
/// ```
pub struct AuditedDatabase<E: StorageEngine> {
    inner: Database<E>,
}

impl<E: StorageEngine> AuditedDatabase<E> {
    /// Create a new audited database wrapper.
    ///
    /// This will automatically create the `_AUDIT_LOG` relvar if it doesn't exist.
    pub fn new(mut inner: Database<E>) -> Result<Self, DatabaseError> {
        if !inner.relvar_exists("_AUDIT_LOG") {
            let audit_type = RelationType::new(
                TupleType::new()
                    .with_attribute("id", ScalarType::Int)
                    .with_attribute("timestamp", ScalarType::Int)
                    .with_attribute("operation", ScalarType::String)
                    .with_attribute("relvar_name", ScalarType::String)
                    .with_attribute("before_image", ScalarType::String)
                    .with_attribute("after_image", ScalarType::String),
            );

            inner.create_relvar("_AUDIT_LOG", audit_type)?;

            // Set Primary Key on id
            let pk = PrimaryKey::new(vec!["id".to_string()]).map_err(|e| {
                DatabaseError::Constraint(ConstraintManagerError::TransactionError(e.to_string()))
            })?;
            let constraints = KeyConstraints::new().with_primary_key(pk);
            inner.set_key_constraints("_AUDIT_LOG", constraints)?;
        }
        Ok(Self { inner })
    }

    /// Access the inner database.
    pub fn inner(&self) -> &Database<E> {
        &self.inner
    }

    /// Access the inner database mutably.
    /// warning: bypassing audit log!
    pub fn inner_mut(&mut self) -> &mut Database<E> {
        &mut self.inner
    }

    /// Create a new base relvar.
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        self.inner.create_relvar(name, relation_type)
    }

    /// Drop a base relvar.
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        // TODO: Log DROP operations?
        self.inner.drop_relvar(name)
    }

    /// Check if a relvar exists.
    pub fn relvar_exists(&self, name: &str) -> bool {
        self.inner.relvar_exists(name)
    }

    /// List all relvars.
    pub fn list_relvars(&self) -> Vec<String> {
        self.inner.list_relvars()
    }

    /// Get relvar type.
    pub fn get_relvar_type(&self, name: &str) -> Result<RelationType, DatabaseError> {
        self.inner.get_relvar_type(name)
    }

    /// Query a relation.
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        self.inner.query(relation_name)
    }

    /// Begin a transaction.
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        self.inner.begin()
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        self.inner.commit()
    }

    /// Rollback the current transaction.
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        self.inner.rollback()
    }

    /// Insert a tuple into a relation.
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        if relation_name == "_AUDIT_LOG" {
            return self.inner.insert(relation_name, tuple);
        }

        let started_transaction = if !self.inner.is_in_transaction() {
            self.inner.begin()?;
            true
        } else {
            false
        };

        // Clone tuple for logging since insert consumes it
        let tuple_for_log = tuple.clone();

        match self.inner.insert(relation_name, tuple) {
            Ok(_) => {
                let log_entry =
                    self.create_log_entry("INSERT", relation_name, None, Some(&tuple_for_log))?;
                match self.inner.insert("_AUDIT_LOG", log_entry) {
                    Ok(_) => {
                        if started_transaction {
                            self.inner.commit()?;
                        }
                        Ok(())
                    }
                    Err(e) => {
                        if started_transaction {
                            let _ = self.inner.rollback();
                        }
                        Err(e)
                    }
                }
            }
            Err(e) => {
                if started_transaction {
                    let _ = self.inner.rollback();
                }
                Err(e)
            }
        }
    }

    fn create_log_entry(
        &self,
        operation: &str,
        relation_name: &str,
        before: Option<&Tuple>,
        after: Option<&Tuple>,
    ) -> Result<Tuple, DatabaseError> {
        use relvar_core::tuple;

        // Generate ID
        let id: i64 = rand::thread_rng().r#gen::<i64>().abs();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let before_json = if let Some(t) = before {
            serde_json::to_string(t).unwrap_or_default()
        } else {
            "null".to_string()
        };

        let after_json = if let Some(t) = after {
            serde_json::to_string(t).unwrap_or_default()
        } else {
            "null".to_string()
        };

        Ok(tuple! {
            id: id,
            timestamp: timestamp,
            operation: operation,
            relvar_name: relation_name,
            before_image: before_json,
            after_image: after_json
        })
    }

    /// Delete tuples matching a predicate.
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool + Clone,
    {
        if relation_name == "_AUDIT_LOG" {
            return self.inner.delete(relation_name, predicate);
        }

        let started_transaction = if !self.inner.is_in_transaction() {
            self.inner.begin()?;
            true
        } else {
            false
        };

        // Query to find affected tuples
        let current_relation = match self.inner.query(relation_name) {
            Ok(rel) => rel,
            Err(e) => {
                if started_transaction {
                    let _ = self.inner.rollback();
                }
                return Err(e);
            }
        };

        let tuples_to_delete: Vec<Tuple> = current_relation
            .tuples()
            .filter(|t| predicate(t))
            .cloned()
            .collect();

        if tuples_to_delete.is_empty() {
            if started_transaction {
                let _ = self.inner.commit();
            }
            return Ok(0);
        }

        match self.inner.delete(relation_name, predicate) {
            Ok(count) => {
                // Log deletions
                for tuple in tuples_to_delete {
                    let log_entry =
                        match self.create_log_entry("DELETE", relation_name, Some(&tuple), None) {
                            Ok(entry) => entry,
                            Err(e) => {
                                if started_transaction {
                                    let _ = self.inner.rollback();
                                }
                                return Err(e);
                            }
                        };
                    if let Err(e) = self.inner.insert("_AUDIT_LOG", log_entry) {
                        if started_transaction {
                            let _ = self.inner.rollback();
                        }
                        return Err(e);
                    }
                }

                if started_transaction {
                    self.inner.commit()?;
                }
                Ok(count)
            }
            Err(e) => {
                if started_transaction {
                    let _ = self.inner.rollback();
                }
                Err(e)
            }
        }
    }

    /// Update tuples matching a predicate.
    pub fn update<F, U>(
        &mut self,
        relation_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool + Clone,
        U: Fn(&Tuple) -> Tuple + Clone,
    {
        if relation_name == "_AUDIT_LOG" {
            return self.inner.update(relation_name, predicate, updater);
        }

        let started_transaction = if !self.inner.is_in_transaction() {
            self.inner.begin()?;
            true
        } else {
            false
        };

        // Query to find affected tuples
        let current_relation = match self.inner.query(relation_name) {
            Ok(rel) => rel,
            Err(e) => {
                if started_transaction {
                    let _ = self.inner.rollback();
                }
                return Err(e);
            }
        };

        let tuples_to_update: Vec<Tuple> = current_relation
            .tuples()
            .filter(|t| predicate(t))
            .cloned()
            .collect();

        if tuples_to_update.is_empty() {
            if started_transaction {
                let _ = self.inner.commit();
            }
            return Ok(0);
        }

        // Calculate after images
        let mut updates_log_data = Vec::new();
        for before in tuples_to_update {
            let after = updater(&before);
            updates_log_data.push((before, after));
        }

        match self.inner.update(relation_name, predicate, updater) {
            Ok(count) => {
                for (before, after) in updates_log_data {
                    let log_entry = match self.create_log_entry(
                        "UPDATE",
                        relation_name,
                        Some(&before),
                        Some(&after),
                    ) {
                        Ok(entry) => entry,
                        Err(e) => {
                            if started_transaction {
                                let _ = self.inner.rollback();
                            }
                            return Err(e);
                        }
                    };
                    if let Err(e) = self.inner.insert("_AUDIT_LOG", log_entry) {
                        if started_transaction {
                            let _ = self.inner.rollback();
                        }
                        return Err(e);
                    }
                }

                if started_transaction {
                    self.inner.commit()?;
                }
                Ok(count)
            }
            Err(e) => {
                if started_transaction {
                    let _ = self.inner.rollback();
                }
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{ScalarType, TupleType};

    fn create_db() -> AuditedDatabase<InMemoryEngine> {
        let db = Database::new(InMemoryEngine::new());
        AuditedDatabase::new(db).unwrap()
    }

    fn test_rel_type() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        )
    }

    #[test]
    fn test_audit_log_created_automatically() {
        let db = create_db();
        assert!(db.relvar_exists("_AUDIT_LOG"));

        let rel_type = db.get_relvar_type("_AUDIT_LOG").unwrap();
        assert!(rel_type.has_attribute("id"));
        assert!(rel_type.has_attribute("timestamp"));
        assert!(rel_type.has_attribute("operation"));
        assert!(rel_type.has_attribute("relvar_name"));
        assert!(rel_type.has_attribute("before_image"));
        assert!(rel_type.has_attribute("after_image"));
    }

    #[test]
    fn test_insert_is_audited() {
        let mut db = create_db();
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        let logs = db.query("_AUDIT_LOG").unwrap();
        assert_eq!(logs.cardinality(), 1);

        let log = logs.tuples().next().unwrap();
        assert_eq!(log.get_typed::<String>("operation").unwrap(), "INSERT");
        assert_eq!(log.get_typed::<String>("relvar_name").unwrap(), "TEST");

        // Check after image contains inserted data
        let after_image = log.get_typed::<String>("after_image").unwrap();
        assert!(after_image.contains("Alice"));
        assert!(after_image.contains("1"));

        // Before image should be "null"
        assert_eq!(log.get_typed::<String>("before_image").unwrap(), "null");
    }

    #[test]
    fn test_delete_is_audited() {
        let mut db = create_db();
        db.create_relvar("TEST", test_rel_type()).unwrap();
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Clear logs from insert
        db.delete("_AUDIT_LOG", |_| true).unwrap();

        db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
            .unwrap();

        let logs = db.query("_AUDIT_LOG").unwrap();
        assert_eq!(logs.cardinality(), 1);

        let log = logs.tuples().next().unwrap();
        assert_eq!(log.get_typed::<String>("operation").unwrap(), "DELETE");

        // Before image should be the deleted tuple
        let before_image = log.get_typed::<String>("before_image").unwrap();
        assert!(before_image.contains("Alice"));

        // After image should be "null"
        assert_eq!(log.get_typed::<String>("after_image").unwrap(), "null");
    }

    #[test]
    fn test_update_is_audited() {
        let mut db = create_db();
        db.create_relvar("TEST", test_rel_type()).unwrap();
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Clear logs
        db.delete("_AUDIT_LOG", |_| true).unwrap();

        db.update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_t| tuple! { id: 1i64, name: "Bob" },
        )
        .unwrap();

        let logs = db.query("_AUDIT_LOG").unwrap();
        assert_eq!(logs.cardinality(), 1);

        let log = logs.tuples().next().unwrap();
        assert_eq!(log.get_typed::<String>("operation").unwrap(), "UPDATE");

        let before_image = log.get_typed::<String>("before_image").unwrap();
        assert!(before_image.contains("Alice"));

        let after_image = log.get_typed::<String>("after_image").unwrap();
        assert!(after_image.contains("Bob"));
    }

    #[test]
    fn test_transaction_rollback() {
        let mut db = create_db();
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.begin().unwrap();
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.rollback().unwrap();

        // Verify no data in TEST
        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 0);

        // Verify no audit logs
        let logs = db.query("_AUDIT_LOG").unwrap();
        assert_eq!(logs.cardinality(), 0);
    }
}
