use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, TupleType};
use relvar_core::database::DatabaseError;

#[test]
fn test_virtual_relvar_infinite_recursion() {
    let mut db = Database::new(InMemoryEngine::new());

    // Define virtual relvar "A" that queries "B"
    db.define_virtual_relvar(
        "A",
        RelationType::new(TupleType::new()),
        |db| db.query("B")
    ).unwrap();

    // Define virtual relvar "B" that queries "A"
    db.define_virtual_relvar(
        "B",
        RelationType::new(TupleType::new()),
        |db| db.query("A")
    ).unwrap();

    // This should return RecursionLimitExceeded instead of crashing
    let result = db.query("A");
    assert!(result.is_err());

    match result {
        Err(DatabaseError::RecursionLimitExceeded(name)) => {
            // "A" calls "B", "B" calls "A". Recursion detected at "A" (second time) or "B" depending on impl
            assert!(name == "A" || name == "B");
        }
        _ => panic!("Expected RecursionLimitExceeded, got {:?}", result),
    }
}
