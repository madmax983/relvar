//! Database assertions on a persistent database (TTM RM Prescriptions 16
//! and 23): a database contains its constraints, so an assertion declared
//! on it must survive closing and reopening it and keep being enforced,
//! with nothing to re-register.

#![cfg(feature = "storage")]

use relvar::constraints::{AssertionCondition, AssertionError, DeclarativeAssertion};
use relvar::{
    Database, DatabaseError, PersistentEngine, Query, RelationType, ScalarType, TupleType, tuple,
};
use std::path::Path;

type Db = Database<PersistentEngine>;

fn open(dir: &Path) -> Db {
    Database::new(PersistentEngine::open(dir).unwrap())
}

/// P{pid} and C{cid, pid}, with P = {1} and C = {(10, 1)}.
fn build_pc(dir: &Path) -> Db {
    let mut db = open(dir);
    db.create_relvar(
        "P",
        RelationType::new(TupleType::new().with_attribute("pid", ScalarType::Int)),
    )
    .unwrap();
    db.create_relvar(
        "C",
        RelationType::new(
            TupleType::new()
                .with_attribute("cid", ScalarType::Int)
                .with_attribute("pid", ScalarType::Int),
        ),
    )
    .unwrap();
    db.insert("P", tuple! { pid: 1i64 }).unwrap();
    db.insert("C", tuple! { cid: 10i64, pid: 1i64 }).unwrap();
    db
}

fn child_has_parent() -> DeclarativeAssertion {
    DeclarativeAssertion::new(
        "child_has_parent",
        "Every child references an existing parent",
        AssertionCondition::IsEmpty(Query::scan("C").not_matching(Query::scan("P"))),
    )
}

fn is_violation<T>(result: &Result<T, DatabaseError>) -> bool {
    matches!(result, Err(DatabaseError::AssertionViolation(message)) if message.contains("child_has_parent"))
}

fn cardinalities(db: &Db) -> (usize, usize) {
    (
        db.query("P").unwrap().cardinality(),
        db.query("C").unwrap().cardinality(),
    )
}

#[test]
fn declarative_assertion_is_enforced_after_reopen_without_re_registration() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = build_pc(dir.path());
        db.add_declarative_assertion(child_has_parent()).unwrap();
    }

    let mut db = open(dir.path());
    assert_eq!(db.declarative_assertions().len(), 1);
    assert!(is_violation(
        &db.insert("C", tuple! { cid: 11i64, pid: 9i64 })
    ));
    assert!(is_violation(&db.delete("P", |_| true)));
    assert!(is_violation(&db.update(
        "C",
        |_| true,
        |_| tuple! { cid: 10i64, pid: 2i64 }
    )));
    db.begin().unwrap();
    assert!(is_violation(
        &db.bulk_insert("C", vec![tuple! { cid: 11i64, pid: 9i64 }])
    ));
    db.insert("P", tuple! { pid: 2i64 }).unwrap();
    db.insert("C", tuple! { cid: 11i64, pid: 2i64 }).unwrap();
    db.commit().unwrap();
    assert_eq!(cardinalities(&db), (2, 2));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn removed_assertion_stays_removed_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = build_pc(dir.path());
        db.add_declarative_assertion(child_has_parent()).unwrap();
        assert!(db.remove_assertion("child_has_parent").unwrap());
    }

    let mut db = open(dir.path());
    assert!(db.declarative_assertions().is_empty());
    db.insert("C", tuple! { cid: 11i64, pid: 9i64 }).unwrap();
}

#[test]
fn rejected_assertion_is_not_stored() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = build_pc(dir.path());
        db.insert("C", tuple! { cid: 11i64, pid: 9i64 }).unwrap();
        assert!(is_violation(
            &db.add_declarative_assertion(child_has_parent())
        ));
    }
    assert!(open(dir.path()).declarative_assertions().is_empty());
}

#[test]
fn catalog_without_database_constraints_still_opens() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = build_pc(dir.path());
        db.add_declarative_assertion(child_has_parent()).unwrap();
    }
    // Rewrite the catalog as a version without database-wide constraints
    // wrote it.
    let catalog_path = dir.path().join("catalog.json");
    let text = std::fs::read_to_string(&catalog_path).unwrap();
    let mut catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
    let object = catalog.as_object_mut().unwrap();
    assert!(object.remove("database_constraints").is_some());
    std::fs::write(&catalog_path, serde_json::to_string(&catalog).unwrap()).unwrap();

    let mut db = open(dir.path());
    assert!(db.declarative_assertions().is_empty());
    assert_eq!(cardinalities(&db), (1, 1));
    db.insert("C", tuple! { cid: 11i64, pid: 9i64 }).unwrap();
}

#[test]
fn dropping_a_relvar_an_assertion_refers_to_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = build_pc(dir.path());
        db.add_declarative_assertion(child_has_parent()).unwrap();
    }

    let mut db = open(dir.path());
    for relvar in ["P", "C"] {
        assert!(matches!(
            db.drop_relvar(relvar),
            Err(DatabaseError::Assertion(AssertionError::RelvarReferenced { relvar: r, .. })) if r == relvar
        ));
    }
    drop(db);

    // Still there, and still enforced.
    let mut db = open(dir.path());
    assert!(db.relvar_exists("P") && db.relvar_exists("C"));
    assert!(is_violation(&db.delete("P", |_| true)));

    // Once the assertion is gone the relvars can be dropped.
    db.remove_assertion("child_has_parent").unwrap();
    db.drop_relvar("C").unwrap();
    db.drop_relvar("P").unwrap();
    drop(db);
    assert!(!open(dir.path()).relvar_exists("P"));
}
