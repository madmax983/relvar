use super::common::*;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
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
