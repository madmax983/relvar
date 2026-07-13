use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;

fn test_rel_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int))
}

#[test]
fn should_return_error_when_modifying_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |_db| Ok(Relation::new(test_rel_type()))).unwrap();

    let result = db.insert("VIRTUAL", tuple! { id: 1i64 });
    assert!(matches!(result, Err(DatabaseError::CannotModifyVirtualRelvar(_))));

    let result = db.delete("VIRTUAL", |_t| true);
    assert!(matches!(result, Err(DatabaseError::CannotModifyVirtualRelvar(_))));

    let result = db.update("VIRTUAL", |_t| true, |t| t.clone());
    assert!(matches!(result, Err(DatabaseError::CannotModifyVirtualRelvar(_))));
}
