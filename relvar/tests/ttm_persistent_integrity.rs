//! Golden Rule regression tests against the persistent engine
//! (TTM RM Prescriptions 16, 21 and 23).
//!
//! relvar-core cannot depend on relvar-storage, so the interplay of the
//! `Database` layer with `PersistentEngine` transactions and catalog is
//! tested here.

#![cfg(feature = "storage")]

use relvar::constraints::DatabaseAssertion;
use relvar::{
    Database, DatabaseError, PersistentEngine, RelationType, ScalarType, ScalarValue, TupleType,
    tuple,
};

type Db = Database<PersistentEngine>;

fn accounts_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("balance", ScalarType::Int),
    )
}

fn total_balance(db: &mut Db) -> i64 {
    db.query("ACCOUNTS")
        .map(|accounts| {
            accounts
                .tuples()
                .filter_map(|t| match t.get("balance") {
                    Some(ScalarValue::Int(n)) => Some(*n),
                    _ => None,
                })
                .sum()
        })
        .unwrap_or(i64::MIN)
}

fn non_negative_total() -> DatabaseAssertion<PersistentEngine> {
    DatabaseAssertion::new(
        "non_negative_total",
        "The sum of all balances is non-negative",
        |db: &mut Db| total_balance(db) >= 0,
    )
}

fn accounts_db(dir: &std::path::Path) -> Db {
    let mut db = Database::new(PersistentEngine::open(dir).unwrap());
    db.create_relvar("ACCOUNTS", accounts_type()).unwrap();
    db
}

// --- Finding 5: assertion guard inside a user transaction -------------------

#[test]
fn rollback_undoes_assertion_guarded_inserts_on_persistent_engine() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = accounts_db(dir.path());
    db.add_assertion(non_negative_total()).unwrap();

    db.begin().unwrap();
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 10i64 })
        .unwrap();
    db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 20i64 })
        .unwrap();
    db.rollback().unwrap();

    assert_eq!(db.query("ACCOUNTS").unwrap().cardinality(), 0);
}

#[test]
fn violating_update_inside_transaction_is_undone_and_transaction_continues() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = accounts_db(dir.path());
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 10i64 })
        .unwrap();
    db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 20i64 })
        .unwrap();
    db.add_assertion(non_negative_total()).unwrap();

    db.begin().unwrap();
    let violating = db.update(
        "ACCOUNTS",
        |_| true,
        |t| tuple! { id: t.get_typed::<i64>("id").unwrap_or_default(), balance: -1i64 },
    );
    assert!(matches!(
        violating,
        Err(DatabaseError::AssertionViolation(_))
    ));
    assert_eq!(total_balance(&mut db), 30);
    db.delete("ACCOUNTS", |t| t.get_typed::<i64>("id") == Some(2))
        .unwrap();
    db.commit().unwrap();

    assert_eq!(total_balance(&mut db), 10);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn assertion_violation_inside_transaction_undoes_only_that_statement() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = accounts_db(dir.path());
    db.add_assertion(non_negative_total()).unwrap();

    db.begin().unwrap();
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 10i64 })
        .unwrap();
    let violating = db.insert("ACCOUNTS", tuple! { id: 2i64, balance: -50i64 });
    assert!(matches!(
        violating,
        Err(DatabaseError::AssertionViolation(_))
    ));
    // The transaction is still open and still holds the first insert.
    assert_eq!(db.query("ACCOUNTS").unwrap().cardinality(), 1);
    db.insert("ACCOUNTS", tuple! { id: 3i64, balance: 5i64 })
        .unwrap();
    db.commit().unwrap();

    assert_eq!(total_balance(&mut db), 15);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn committed_assertion_guarded_transaction_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = accounts_db(dir.path());
        db.add_assertion(non_negative_total()).unwrap();
        db.begin().unwrap();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 10i64 })
            .unwrap();
        db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 20i64 })
            .unwrap();
        db.commit().unwrap();
    }
    let mut db = Database::new(PersistentEngine::open(dir.path()).unwrap());
    assert_eq!(total_balance(&mut db), 30);
}
