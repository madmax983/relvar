use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;

#[test]
fn test_get_constraints_not_found() {
    let db = Database::new(InMemoryEngine::new());

    assert!(db.get_key_constraints("MISSING").is_none());
    assert!(db.get_foreign_key_constraints("MISSING").is_none());
}
