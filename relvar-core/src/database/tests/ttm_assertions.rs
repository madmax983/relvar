//! Assertion enforcement inside user transactions (RM Prescription 23).
//!
//! The persistent-engine counterpart lives in
//! `relvar/tests/ttm_persistent_integrity.rs`.

use super::golden_rule::id_name_type;
use crate::constraints::DatabaseAssertion;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;

fn at_most_two() -> DatabaseAssertion<InMemoryEngine> {
    DatabaseAssertion::new(
        "at_most_two",
        "R holds at most two tuples",
        |db: &mut Database<InMemoryEngine>| {
            db.query("R").map(|r| r.cardinality() <= 2).unwrap_or(false)
        },
    )
}

fn db_with_assertion() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.add_assertion(at_most_two()).unwrap();
    db
}

#[test]
fn rollback_undoes_assertion_guarded_statements() {
    let mut db = db_with_assertion();
    db.begin().unwrap();
    db.insert("R", tuple! { id: 1i64, name: "a" }).unwrap();
    db.bulk_insert("R", alloc::vec![tuple! { id: 2i64, name: "b" }])
        .unwrap();
    db.rollback().unwrap();
    assert!(db.query("R").unwrap().is_empty());
}

#[test]
fn rollback_undoes_assertion_guarded_update_and_delete() {
    let mut db = db_with_assertion();
    db.insert("R", tuple! { id: 1i64, name: "a" }).unwrap();
    db.insert("R", tuple! { id: 2i64, name: "b" }).unwrap();
    db.begin().unwrap();
    db.update(
        "R",
        |_| true,
        |t| {
            let mut renamed = t.clone();
            let _ = renamed.set(
                "name".into(),
                crate::values::ScalarValue::String("z".into()),
            );
            renamed
        },
    )
    .unwrap();
    db.delete("R", |_| true).unwrap();
    db.rollback().unwrap();
    assert_eq!(db.query("R").unwrap().cardinality(), 2);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn violation_inside_transaction_undoes_only_the_statement() {
    let mut db = db_with_assertion();
    db.begin().unwrap();
    db.insert("R", tuple! { id: 1i64, name: "a" }).unwrap();
    db.insert("R", tuple! { id: 2i64, name: "b" }).unwrap();
    let third = db.insert("R", tuple! { id: 3i64, name: "c" });
    assert!(matches!(third, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(db.query("R").unwrap().cardinality(), 2);
    db.commit().unwrap();
    assert_eq!(db.query("R").unwrap().cardinality(), 2);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn assertion_predicates_cannot_perform_dml() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    // The predicate only holds if its own write attempt is refused.
    db.add_assertion(DatabaseAssertion::new(
        "read_only",
        "Assertion predicates are read-only",
        |db: &mut Database<InMemoryEngine>| {
            matches!(
                db.insert("R", tuple! { id: 99i64, name: "sneaky" }),
                Err(DatabaseError::TransactionError(_))
            )
        },
    ))
    .unwrap();
    db.insert("R", tuple! { id: 1i64, name: "a" }).unwrap();
    assert_eq!(db.query("R").unwrap().cardinality(), 1);
}
