use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;

#[test]
fn test_drop_relvar_not_found() {
    let mut db = Database::new(InMemoryEngine::new());

    let result = db.drop_relvar("NONEXISTENT");
    assert!(result.is_err());
}
