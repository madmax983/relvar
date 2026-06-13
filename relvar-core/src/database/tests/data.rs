use super::common::*;
use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;

#[test]
fn test_delete() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let deleted = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();
    assert_eq!(deleted, 1);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_update() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let updated = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |t| tuple! { id: t.get_typed::<i64>("id").unwrap(), name: "Alicia" },
        )
        .unwrap();
    assert_eq!(updated, 1);

    let result = db.query("TEST").unwrap();
    let tuple = result.tuples().next().unwrap();
    assert_eq!(tuple.get_typed::<String>("name").unwrap(), "Alicia");
}

#[test]
fn test_delete_returns_count() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    db.insert("TEST", tuple! { id: 3i64, name: "Charlie" })
        .unwrap();

    // Delete some tuples
    let count = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 1)
        .unwrap();
    assert_eq!(count, 2);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_update_returns_count() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Update tuples
    let count = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| {
                tuple! { id: 1i64, name: "Alicia" }
            },
        )
        .unwrap();

    assert_eq!(count, 1);

    let result = db.query("TEST").unwrap();
    assert!(result.contains(&tuple! { id: 1i64, name: "Alicia" }));
}

#[test]
fn test_delete_no_matches_returns_zero() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Delete with no matches
    let count = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 100)
        .unwrap();
    assert_eq!(count, 0);

    // Relation should be unchanged
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_insert_into_nonexistent_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let result = db.insert("NONEXISTENT", tuple! { id: 1i64, name: "Alice" });
    assert!(result.is_err());
}

#[test]
fn test_insert_into_transaction_rollback() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.begin().unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.rollback().unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_delete_from_nonexistent_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let result = db.delete("NONEXISTENT", |_| true);
    assert!(result.is_err());
}

#[test]
fn test_update_nonexistent_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let result = db.update("NONEXISTENT", |_| true, |t| t.clone());
    assert!(result.is_err());
}

#[test]
fn test_update_tuple_mismatch() {
    use crate::error::DatabaseError;
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_| tuple! { id: "not_an_int" }, // tuple mismatch!
    );

    assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
}

#[test]
fn test_update_no_matches_returns_zero() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let count = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() > 100,
            |t| t.clone(),
        )
        .unwrap();

    assert_eq!(count, 0);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_ensure_not_virtual_insert() {
    use crate::error::DatabaseError;
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("BASE", test_rel_type()).unwrap();
    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
        .unwrap();

    let result = db.ensure_not_virtual("VIRTUAL");
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));

    let result = db.ensure_not_virtual("BASE");
    assert!(result.is_ok());
}

#[test]
fn test_query_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("TEST"))
        .unwrap();

    let result = db.query("VIRTUAL").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_query_nonexistent_relvar() {
    let db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let result = db.query("NONEXISTENT");
    assert!(result.is_err());
}
