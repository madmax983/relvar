//! Tests for database assertion enforcement on DML operations (#36).

use crate::constraints::DatabaseAssertion;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::ScalarValue;

fn accounts_db() -> Database<InMemoryEngine> {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("balance", ScalarType::Int),
    );
    db.create_relvar("ACCOUNTS", rel_type).unwrap();
    db
}

fn total_balance(db: &mut Database<InMemoryEngine>) -> i64 {
    db.query("ACCOUNTS")
        .unwrap()
        .tuples()
        .filter_map(|t| t.get("balance"))
        .filter_map(|v| {
            if let ScalarValue::Int(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .sum()
}

fn total_non_negative_assertion() -> DatabaseAssertion<InMemoryEngine> {
    DatabaseAssertion::new(
        "total_non_negative",
        "Sum of all account balances must be non-negative",
        |db: &mut Database<InMemoryEngine>| total_balance(db) >= 0,
    )
}

fn users_db() -> Database<InMemoryEngine> {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("role", ScalarType::String),
    );
    db.create_relvar("USERS", rel_type).unwrap();
    db.insert("USERS", tuple! { id: 1i64, role: "admin" })
        .unwrap();
    db.insert("USERS", tuple! { id: 2i64, role: "viewer" })
        .unwrap();
    db
}

fn at_least_one_admin_assertion() -> DatabaseAssertion<InMemoryEngine> {
    DatabaseAssertion::new(
        "at_least_one_admin",
        "Database must have at least one admin user",
        |db: &mut Database<InMemoryEngine>| {
            db.query("USERS").unwrap().tuples().any(|user| {
                matches!(
                    user.get("role"),
                    Some(ScalarValue::String(role)) if role == "admin"
                )
            })
        },
    )
}

#[test]
fn test_insert_enforces_assertions() {
    let mut db = accounts_db();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    // Insert that drives the total negative must fail.
    let result = db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -1000000i64 });
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));

    // Verify the tuple was rolled back: it must not be present.
    let accounts = db.query("ACCOUNTS").unwrap();
    assert_eq!(accounts.cardinality(), 0);
}

#[test]
fn test_insert_satisfying_assertion_succeeds() {
    let mut db = accounts_db();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 50i64 })
        .unwrap();
    assert_eq!(db.query("ACCOUNTS").unwrap().cardinality(), 1);
}

#[test]
fn test_insert_violation_rolls_back_prior_tuples() {
    let mut db = accounts_db();
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
        .unwrap();
    db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 50i64 })
        .unwrap();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    let result = db.insert("ACCOUNTS", tuple! { id: 3i64, balance: -1000000i64 });
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));

    // Pre-existing tuples must be untouched by the rolled-back insert.
    let accounts = db.query("ACCOUNTS").unwrap();
    assert_eq!(accounts.cardinality(), 2);
    assert_eq!(total_balance(&mut db), 150);
}

#[test]
fn test_update_enforces_assertions() {
    let mut db = accounts_db();
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
        .unwrap();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    // Update that drives the total negative must fail.
    let result = db.update(
        "ACCOUNTS",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_| tuple! { id: 1i64, balance: -50i64 },
    );
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));

    // The tuple must keep its original value: the update was rolled back.
    let accounts = db.query("ACCOUNTS").unwrap();
    assert_eq!(accounts.cardinality(), 1);
    let balance = accounts
        .tuples()
        .next()
        .unwrap()
        .get_typed::<i64>("balance")
        .unwrap();
    assert_eq!(balance, 100);
}

#[test]
fn test_update_satisfying_assertion_succeeds() {
    let mut db = accounts_db();
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
        .unwrap();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    let updated = db
        .update(
            "ACCOUNTS",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| tuple! { id: 1i64, balance: 200i64 },
        )
        .unwrap();
    assert_eq!(updated, 1);
    assert_eq!(total_balance(&mut db), 200);
}

#[test]
fn test_delete_enforces_assertions() {
    let mut db = users_db();
    db.add_assertion(at_least_one_admin_assertion()).unwrap();

    // Deleting the only admin must fail.
    let result = db.delete("USERS", |u| {
        matches!(
            u.get("role"),
            Some(ScalarValue::String(role)) if role == "admin"
        )
    });
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));

    // Verify the tuple was not deleted.
    let users = db.query("USERS").unwrap();
    assert_eq!(users.cardinality(), 2);
    assert!(users.tuples().any(|u| matches!(
        u.get("role"),
        Some(ScalarValue::String(role)) if role == "admin"
    )));
}

#[test]
fn test_delete_satisfying_assertion_succeeds() {
    let mut db = users_db();
    db.add_assertion(at_least_one_admin_assertion()).unwrap();

    // Deleting a non-admin keeps the assertion satisfied.
    let deleted = db
        .delete("USERS", |u| {
            matches!(
                u.get("role"),
                Some(ScalarValue::String(role)) if role == "viewer"
            )
        })
        .unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(db.query("USERS").unwrap().cardinality(), 1);
}

#[test]
fn test_assertion_sees_post_op_state() {
    let mut db = accounts_db();
    db.add_assertion(DatabaseAssertion::new(
        "at_most_one_account",
        "The ACCOUNTS relvar holds at most one tuple",
        |db: &mut Database<InMemoryEngine>| db.query("ACCOUNTS").unwrap().cardinality() <= 1,
    ))
    .unwrap();

    // First insert is fine (post-op cardinality is 1).
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 10i64 })
        .unwrap();

    // Second insert sees the post-op state (cardinality 2) and must fail.
    let result = db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 20i64 });
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(db.query("ACCOUNTS").unwrap().cardinality(), 1);
}

#[test]
fn test_multiple_assertions_all_checked() {
    let mut db = accounts_db();
    // First assertion always holds; the violation must come from the second.
    db.add_assertion(DatabaseAssertion::new(
        "always_true",
        "Trivially satisfied assertion",
        |_: &mut Database<InMemoryEngine>| true,
    ))
    .unwrap();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    let result = db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -5i64 });
    let message = match result {
        Err(DatabaseError::AssertionViolation(message)) => message,
        _ => panic!("Expected AssertionViolation"),
    };
    assert!(
        message.contains("total_non_negative"),
        "violation names the second assertion: {message}"
    );
}

#[test]
fn test_assertion_error_message() {
    let mut db = accounts_db();
    db.add_assertion(total_non_negative_assertion()).unwrap();

    let result = db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -5i64 });
    let message = match result {
        Err(DatabaseError::AssertionViolation(message)) => message,
        _ => panic!("Expected AssertionViolation"),
    };
    assert!(
        message.contains("total_non_negative"),
        "message names the assertion: {message}"
    );
    assert!(
        message.contains("Sum of all account balances must be non-negative"),
        "message describes the rule: {message}"
    );
}

#[test]
fn test_remove_assertion_stops_enforcement() {
    let mut db = accounts_db();
    db.add_assertion(total_non_negative_assertion()).unwrap();
    assert!(db.remove_assertion("total_non_negative"));

    // With the assertion gone, the previously-violating insert succeeds.
    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -1000000i64 })
        .unwrap();
    assert_eq!(db.query("ACCOUNTS").unwrap().cardinality(), 1);
}

#[test]
fn test_no_assertions_registered_ops_unaffected() {
    let mut db = accounts_db();

    db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
        .unwrap();
    let updated = db
        .update(
            "ACCOUNTS",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| tuple! { id: 1i64, balance: 250i64 },
        )
        .unwrap();
    assert_eq!(updated, 1);
    let deleted = db
        .delete("ACCOUNTS", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(db.query("ACCOUNTS").unwrap().cardinality(), 0);
}
