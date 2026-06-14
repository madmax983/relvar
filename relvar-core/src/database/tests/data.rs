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
fn test_update_mismatch_returns_error() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Update tuples
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_| {
            tuple! { id: "hello" }
        },
    );

    assert!(result.is_err());
}

#[test]
fn test_update_no_matches_returns_zero() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Update with no matches
    let count = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() > 100,
            |t| t.clone(),
        )
        .unwrap();
    assert_eq!(count, 0);

    // Relation should be unchanged
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}
