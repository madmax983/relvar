use super::common::*;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::{InMemoryEngine, StorageError};
use crate::tuple;
use crate::values::Relation;

#[test]
fn test_create_and_query_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    assert!(db.relvar_exists("TEST"));

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_drop_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.drop_relvar("TEST").unwrap();

    assert!(!db.relvar_exists("TEST"));
}

#[test]
fn test_error_relvar_not_found() {
    let db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.query("NONEXISTENT");
    assert!(result.is_err());
    // Error should be Storage(RelationNotFound) since it comes from the engine
    match result {
        Err(DatabaseError::Storage(StorageError::RelationNotFound(name))) => {
            assert_eq!(name, "NONEXISTENT");
        }
        other => panic!("Expected RelationNotFound error, got: {:?}", other),
    }
}

#[test]
fn test_error_duplicate_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let result = db.create_relvar("TEST", test_rel_type());
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
    ));
}

#[test]
fn test_get_relvar_type() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Test base relvar
    db.create_relvar("BASE", test_rel_type()).unwrap();
    let base_type = db.get_relvar_type("BASE").unwrap();
    assert_eq!(base_type, test_rel_type());

    // Test virtual relvar
    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
        .unwrap();
    let virtual_type = db.get_relvar_type("VIRTUAL").unwrap();
    assert_eq!(virtual_type, test_rel_type());

    // Test missing relvar
    let result = db.get_relvar_type("NONEXISTENT");
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Storage(_))));
}

#[test]
fn test_list_relvars() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Initially empty
    assert!(db.list_relvars().is_empty());

    let rel_type = test_rel_type();

    // Add base relvars
    db.create_relvar("TABLE_A", rel_type.clone()).unwrap();
    db.create_relvar("TABLE_B", rel_type.clone()).unwrap();

    let mut relvars = db.list_relvars();
    relvars.sort();
    assert_eq!(relvars, vec!["TABLE_A", "TABLE_B"]);

    // Add virtual relvar
    db.define_virtual_relvar("VIEW_C", rel_type.clone(), |_| {
        Ok(Relation::new(test_rel_type()))
    })
    .unwrap();

    let mut relvars2 = db.list_relvars();
    relvars2.sort();
    assert_eq!(relvars2, vec!["TABLE_A", "TABLE_B", "VIEW_C"]);

    // Drop one
    db.drop_relvar("TABLE_A").unwrap();
    let mut relvars3 = db.list_relvars();
    relvars3.sort();
    assert_eq!(relvars3, vec!["TABLE_B", "VIEW_C"]);
}
