use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_drop_relvar_not_found() {
    let mut db = Database::new(InMemoryEngine::new());
    let result = db.drop_relvar("NONEXISTENT");
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Storage(_))));
}

#[test]
fn test_get_relvar_type_not_found() {
    let db = Database::new(InMemoryEngine::new());
    let result = db.get_relvar_type("NONEXISTENT");
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Storage(_))));
}

#[test]
fn test_get_relvar_type_virtual() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.define_virtual_relvar("VIRT_TEST", rel_type.clone(), |_| {
        Ok(relvar_core::values::Relation::new(
            relvar_core::types::RelationType::new(relvar_core::types::TupleType::new()),
        ))
    })
    .unwrap();

    let result = db.get_relvar_type("VIRT_TEST");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), rel_type);
}

#[test]
fn test_create_relvar_already_exists() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));

    // Create first time
    db.create_relvar("TEST", rel_type.clone()).unwrap();

    // Create second time should fail
    let result = db.create_relvar("TEST", rel_type);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
    ));
}

#[test]
fn test_define_virtual_relvar_already_exists_base() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));

    // Create base relvar
    db.create_relvar("TEST", rel_type.clone()).unwrap();

    // Define virtual relvar with same name should fail
    let result = db.define_virtual_relvar("TEST", rel_type, |_| {
        Ok(relvar_core::values::Relation::new(
            relvar_core::types::RelationType::new(relvar_core::types::TupleType::new()),
        ))
    });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
    ));
}

#[test]
fn test_define_virtual_relvar_already_exists_virtual() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));

    db.define_virtual_relvar("VIRT_TEST", rel_type.clone(), |_| {
        Ok(relvar_core::values::Relation::new(
            relvar_core::types::RelationType::new(relvar_core::types::TupleType::new()),
        ))
    })
    .unwrap();

    // Define virtual relvar with same name should fail
    let result = db.define_virtual_relvar("VIRT_TEST", rel_type, |_| {
        Ok(relvar_core::values::Relation::new(
            relvar_core::types::RelationType::new(relvar_core::types::TupleType::new()),
        ))
    });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
    ));
}
