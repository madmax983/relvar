use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};

fn test_rel_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int))
}

#[test]
fn should_return_error_when_defining_duplicate_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
        .unwrap();

    let result = db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"));
    assert!(result.is_err());
}

#[test]
fn should_return_error_when_dropping_nonexistent_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let result = db.drop_virtual_relvar("NONEXISTENT");
    assert!(result.is_err());
}
