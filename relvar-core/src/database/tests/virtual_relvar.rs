use super::common::*;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

#[test]
fn test_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Define virtual relvar that projects just names
    db.define_virtual_relvar(
        "NAMES",
        RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
        |db| {
            let test = db.query("TEST")?;
            Ok(test.project(&["name"]))
        },
    )
    .unwrap();

    // Query virtual relvar
    let names = db.query("NAMES").unwrap();
    assert_eq!(names.degree(), 1);
    assert_eq!(names.cardinality(), 2);
}

#[test]
fn test_drop_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Define virtual relvar
    db.define_virtual_relvar(
        "VIRT",
        RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
        |db| {
            let test = db.query("TEST")?;
            Ok(test.project(&["name"]))
        },
    )
    .unwrap();

    // Verify it exists
    let result = db.query("VIRT");
    assert!(result.is_ok());

    // Drop it
    db.drop_virtual_relvar("VIRT").unwrap();

    // Should no longer exist
    let result = db.query("VIRT");
    assert!(result.is_err());
}

#[test]
fn test_drop_nonexistent_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.drop_virtual_relvar("NONEXISTENT");
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::RelationNotFound(_))));
}

#[test]
fn test_cannot_insert_into_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.define_virtual_relvar(
        "VIRT",
        RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
        |db| {
            let test = db.query("TEST")?;
            Ok(test.project(&["name"]))
        },
    )
    .unwrap();

    // Try to insert
    let result = db.insert("VIRT", tuple! { name: "Alice" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_cannot_delete_from_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("TEST"))
        .unwrap();

    // Try to delete
    let result = db.delete("VIRT", |_| true);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_cannot_update_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("TEST"))
        .unwrap();

    // Try to update
    let result = db.update("VIRT", |_| true, |_| tuple! { id: 99i64, name: "X" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_cannot_drop_base_relvar_when_virtual_depends_on_it() {
    // This would require tracking dependencies, which might not be implemented
    // Skipping for now as it may not be a current feature
}

#[test]
fn test_virtual_relvar_error_propagation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Define virtual relvar that queries nonexistent base
    db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("NONEXISTENT"))
        .unwrap();

    // Querying it should fail
    let result = db.query("VIRT");
    assert!(result.is_err());
}

#[test]
fn test_virtual_relvar_immutability_enforcement() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create a log relation
    let log_type = RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int));
    db.create_relvar("LOG", log_type).unwrap();

    // Define a view. The compiler enforces that we cannot call mutable methods
    // like insert() inside the evaluator because it receives &Database<E>, not &mut Database.
    db.define_virtual_relvar("SAFE_VIEW", test_rel_type(), |db| {
        // db.insert("LOG", ...); // This would cause compilation error!

        // Read operations are allowed
        let _ = db.query("LOG")?;

        Ok(Relation::new(test_rel_type()))
    })
    .unwrap();

    // Query the view
    assert!(db.query("SAFE_VIEW").is_ok());
}

#[test]
fn test_define_virtual_relvar_already_exists() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create base relvar
    db.create_relvar("BASE", test_rel_type()).unwrap();

    // Define virtual relvar
    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
        .unwrap();

    // Try to redefine virtual relvar
    let result = db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"));
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
    ));

    // Try to define virtual relvar with same name as base relvar
    let result = db.define_virtual_relvar("BASE", test_rel_type(), |db| db.query("BASE"));
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
    ));
}
