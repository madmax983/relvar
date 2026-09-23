use super::common::*;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::{InMemoryEngine, IsolationLevel};
use crate::tuple;

#[test]
fn test_transactions() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.begin().unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 2);

    db.rollback().unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_transaction_rollback_with_constraints() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", constraints).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.begin().unwrap();

    // Make some changes
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);

    // Rollback
    db.rollback().unwrap();

    // Should be back to original state
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
    assert!(result.contains(&tuple! { id: 1i64, name: "Alice" }));
}

#[test]
fn test_transaction_commit() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Begin transaction
    db.begin().unwrap();

    // Make changes
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Commit
    db.commit().unwrap();

    // Changes should persist
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 2);

    // Transaction should no longer be active
    assert!(!db.in_transaction);
}

#[test]
fn test_commit_without_transaction() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.commit();
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
}

#[test]
fn test_rollback_without_transaction() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.rollback();
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
}

#[test]
fn test_begin_nested_transaction_fails() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.begin().unwrap();

    // Try to begin another transaction
    let result = db.begin();
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
}

#[test]
fn test_transaction_errors() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Test commit when not in transaction
    let result = db.commit();
    assert!(result.is_err());
    assert!(
        matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "No transaction in progress")
    );

    // Test rollback when not in transaction
    let result = db.rollback();
    assert!(result.is_err());
    assert!(
        matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "No transaction in progress")
    );

    // Test nested begin
    db.begin().unwrap();
    let result = db.begin();
    assert!(result.is_err());
    assert!(
        matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "Transaction already in progress")
    );
}

#[test]
fn test_begin_defaults_to_repeatable_read() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    assert_eq!(db.isolation_level(), None);

    db.begin().unwrap();
    assert_eq!(
        db.isolation_level(),
        Some(IsolationLevel::RepeatableRead),
        "begin() must preserve the historical default isolation level"
    );
    db.commit().unwrap();
    assert_eq!(db.isolation_level(), None);
}

#[test]
fn test_begin_transaction_with_level() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    for level in [
        IsolationLevel::ReadCommitted,
        IsolationLevel::RepeatableRead,
        IsolationLevel::Serializable,
    ] {
        db.begin_transaction_with_level(level).unwrap();
        assert_eq!(db.isolation_level(), Some(level));
        db.commit().unwrap();
        assert_eq!(db.isolation_level(), None);
    }
}

#[test]
fn test_begin_with_level_rejects_nested_transaction() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.begin_transaction_with_level(IsolationLevel::Serializable)
        .unwrap();

    let result = db.begin_transaction_with_level(IsolationLevel::ReadCommitted);
    assert!(result.is_err());
    assert!(
        matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "Transaction already in progress")
    );
    // The original transaction (and its level) is untouched.
    assert_eq!(db.isolation_level(), Some(IsolationLevel::Serializable));
    db.rollback().unwrap();
}

#[test]
fn test_rollback_clears_isolation_level() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.begin_transaction_with_level(IsolationLevel::ReadCommitted)
        .unwrap();
    db.rollback().unwrap();
    assert_eq!(db.isolation_level(), None);
}

#[test]
fn test_transaction_with_level_still_rolls_back_data() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.begin_transaction_with_level(IsolationLevel::Serializable)
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    assert_eq!(db.query("TEST").unwrap().cardinality(), 2);
    db.rollback().unwrap();
    assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
}

/// Engine wrapper that records every isolation level handed to
/// `begin_transaction_with_isolation`, so tests can prove `Database`
/// forwards the caller's chosen level to the engine (rather than merely
/// storing it for `isolation_level()`).
struct LevelRecordingEngine {
    inner: InMemoryEngine,
    seen_levels: Vec<IsolationLevel>,
}

impl LevelRecordingEngine {
    fn new() -> Self {
        Self {
            inner: InMemoryEngine::new(),
            seen_levels: Vec::new(),
        }
    }
}

impl crate::storage_engine::StorageEngine for LevelRecordingEngine {
    type Snapshot = <InMemoryEngine as crate::storage_engine::StorageEngine>::Snapshot;

    fn create_relation(
        &mut self,
        name: &str,
        relation_type: crate::types::RelationType,
    ) -> Result<(), crate::storage_engine::StorageError> {
        self.inner.create_relation(name, relation_type)
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), crate::storage_engine::StorageError> {
        self.inner.drop_relation(name)
    }

    fn relation_exists(&self, name: &str) -> bool {
        self.inner.relation_exists(name)
    }

    fn get_relation_metadata(
        &self,
        name: &str,
    ) -> Result<crate::storage_engine::RelationMetadata, crate::storage_engine::StorageError> {
        self.inner.get_relation_metadata(name)
    }

    fn list_relations(&self) -> Vec<String> {
        self.inner.list_relations()
    }

    fn load_relation(
        &self,
        name: &str,
    ) -> Result<crate::values::Relation, crate::storage_engine::StorageError> {
        self.inner.load_relation(name)
    }

    fn store_relation(
        &mut self,
        name: &str,
        relation: &crate::values::Relation,
    ) -> Result<(), crate::storage_engine::StorageError> {
        self.inner.store_relation(name, relation)
    }

    fn insert_tuple(
        &mut self,
        name: &str,
        tuple: crate::values::Tuple,
    ) -> Result<(), crate::storage_engine::StorageError> {
        self.inner.insert_tuple(name, tuple)
    }

    fn begin_transaction(&mut self) -> Result<Self::Snapshot, crate::storage_engine::StorageError> {
        self.inner.begin_transaction()
    }

    fn begin_transaction_with_isolation(
        &mut self,
        level: IsolationLevel,
    ) -> Result<Self::Snapshot, crate::storage_engine::StorageError> {
        self.seen_levels.push(level);
        self.inner.begin_transaction_with_isolation(level)
    }

    fn commit_transaction(
        &mut self,
        snapshot: Self::Snapshot,
    ) -> Result<(), crate::storage_engine::StorageError> {
        self.inner.commit_transaction(snapshot)
    }

    fn rollback_transaction(
        &mut self,
        snapshot: Self::Snapshot,
    ) -> Result<(), crate::storage_engine::StorageError> {
        self.inner.rollback_transaction(snapshot)
    }
}

#[test]
fn test_begin_with_level_forwards_level_to_engine() {
    let mut db = Database::new(LevelRecordingEngine::new());

    // `begin()` must reach the engine as the default level.
    db.begin().unwrap();
    db.commit().unwrap();

    for level in [
        IsolationLevel::ReadCommitted,
        IsolationLevel::RepeatableRead,
        IsolationLevel::Serializable,
    ] {
        db.begin_transaction_with_level(level).unwrap();
        db.commit().unwrap();
    }

    // The engine observed every level the caller chose, in order: the
    // choice is genuinely forwarded, not just stored on `Database`.
    assert_eq!(
        db.engine.seen_levels,
        vec![
            IsolationLevel::RepeatableRead, // begin() default
            IsolationLevel::ReadCommitted,
            IsolationLevel::RepeatableRead,
            IsolationLevel::Serializable,
        ]
    );
}
