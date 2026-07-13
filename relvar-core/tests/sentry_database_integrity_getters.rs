use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_database_integrity_getters_missing() {
    let mut db = Database::new(InMemoryEngine::new());
    let type1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("A", type1.clone()).unwrap();

    assert!(db.get_key_constraints("A").is_none());
    assert!(db.get_foreign_key_constraints("A").is_none());
}
