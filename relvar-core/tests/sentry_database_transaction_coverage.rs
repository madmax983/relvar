use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};

fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

#[test]
fn test_transaction_errors() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Commit without transaction
    let result = db.commit();
    assert!(matches!(
        result,
        Err(DatabaseError::TransactionError(msg)) if msg == "No transaction in progress"
    ));

    // Rollback without transaction
    let result = db.rollback();
    assert!(matches!(
        result,
        Err(DatabaseError::TransactionError(msg)) if msg == "No transaction in progress"
    ));

    // Double begin
    db.begin().unwrap();
    let result = db.begin();
    assert!(matches!(
        result,
        Err(DatabaseError::TransactionError(msg)) if msg == "Transaction already in progress"
    ));

    // Clean up to keep db in valid state if needed
    db.rollback().unwrap();
}

#[test]
fn test_virtual_relvar_evaluation_error() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Define a virtual relvar that explicitly returns an error
    db.define_virtual_relvar("VIRT_ERR", test_rel_type(), |_db| {
        Err(DatabaseError::TransactionError(
            "Simulated error".to_string(),
        ))
    })
    .unwrap();

    let result = db.query("VIRT_ERR");
    assert!(matches!(
        result,
        Err(DatabaseError::TransactionError(msg)) if msg == "Simulated error"
    ));
}
