use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, TupleType};

#[test]
fn test_drop_relvar_not_found() {
    let mut db = Database::new(InMemoryEngine::new());
    let result = db.drop_relvar("NONEXISTENT");
    assert!(result.is_err());
}

#[test]
fn test_drop_virtual_relvar_not_found() {
    let mut db = Database::new(InMemoryEngine::new());
    let result = db.drop_virtual_relvar("NONEXISTENT");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(relvar_core::error::DatabaseError::RelationNotFound(msg)) if msg == "NONEXISTENT")
    );
}

#[test]
fn test_define_virtual_relvar() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    let result = db.define_virtual_relvar("MY_VIEW", rt.clone(), |_| {
        Ok(relvar_core::values::Relation::new(RelationType::new(
            TupleType::new(),
        )))
    });
    assert!(result.is_ok());
    assert!(db.relvar_exists("MY_VIEW"));
}

#[test]
fn test_create_relvar_already_exists() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    db.create_relvar("TEST", rt.clone()).unwrap();

    let result = db.create_relvar("TEST", rt);
    assert!(result.is_err());
    assert!(
        matches!(result, Err(relvar_core::error::DatabaseError::RelationAlreadyExists(msg)) if msg == "TEST")
    );
}

#[test]
fn test_create_relvar_already_exists_as_virtual() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    db.define_virtual_relvar("TEST", rt.clone(), |_| {
        Ok(relvar_core::values::Relation::new(RelationType::new(
            TupleType::new(),
        )))
    })
    .unwrap();

    let result = db.create_relvar("TEST", rt);
    assert!(result.is_err());
    assert!(
        matches!(result, Err(relvar_core::error::DatabaseError::RelationAlreadyExists(msg)) if msg == "TEST")
    );
}

#[test]
fn test_define_virtual_relvar_already_exists_err() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    db.create_relvar("MY_VIEW", rt.clone()).unwrap();

    let result = db.define_virtual_relvar("MY_VIEW", rt.clone(), |_| {
        Ok(relvar_core::values::Relation::new(RelationType::new(
            TupleType::new(),
        )))
    });
    assert!(result.is_err());
    assert!(
        matches!(result, Err(relvar_core::error::DatabaseError::RelationAlreadyExists(msg)) if msg == "MY_VIEW")
    );
}

#[test]
fn test_drop_virtual_relvar_success() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    db.define_virtual_relvar("MY_VIEW", rt.clone(), |_| {
        Ok(relvar_core::values::Relation::new(RelationType::new(
            TupleType::new(),
        )))
    })
    .unwrap();

    let result = db.drop_virtual_relvar("MY_VIEW");
    assert!(result.is_ok());
    assert!(!db.relvar_exists("MY_VIEW"));
}

#[test]
fn test_list_relvars() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    db.create_relvar("TEST1", rt.clone()).unwrap();
    db.define_virtual_relvar("TEST2", rt.clone(), |_| {
        Ok(relvar_core::values::Relation::new(RelationType::new(
            TupleType::new(),
        )))
    })
    .unwrap();

    let mut relvars = db.list_relvars();
    relvars.sort();
    assert_eq!(relvars, vec!["TEST1", "TEST2"]);
}

#[test]
fn test_relvar_exists() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    db.create_relvar("TEST1", rt.clone()).unwrap();
    db.define_virtual_relvar("TEST2", rt.clone(), |_| {
        Ok(relvar_core::values::Relation::new(RelationType::new(
            TupleType::new(),
        )))
    })
    .unwrap();

    assert!(db.relvar_exists("TEST1"));
    assert!(db.relvar_exists("TEST2"));
    assert!(!db.relvar_exists("TEST3"));
}

#[test]
fn test_create_relvar_success_coverage() {
    let mut db = Database::new(InMemoryEngine::new());
    let rt = RelationType::new(TupleType::new());
    let result = db.create_relvar("TEST1", rt);
    assert!(result.is_ok());
}
