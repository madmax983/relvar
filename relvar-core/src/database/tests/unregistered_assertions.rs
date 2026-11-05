//! Closure assertions are never silently lost (TTM RM Prescriptions 16
//! and 23).
//!
//! A closure-backed [`DatabaseAssertion`] cannot be stored, but its *name*
//! is. A database reopened with a recorded closure assertion that has not
//! been registered again does not know its whole database constraint, so
//! it refuses every change until the assertion is registered again or
//! explicitly forgotten. Queries keep working.

use super::constraint_persistence::{CatalogEngine, reopen};
use super::golden_rule::pk;
use crate::constraints::{
    AssertionCondition, AssertionError, AttributeConstraints, CheckConstraints, DatabaseAssertion,
    DeclarativeAssertion, ForeignKeyConstraints, TypeConstraint,
};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::query::Query;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

type Db = Database<CatalogEngine>;

fn n_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("n", ScalarType::Int))
}

/// N{n} keyed on n, holding {1, 2}.
fn n_db() -> Db {
    let mut db = Database::new(CatalogEngine::default());
    db.create_relvar("N", n_type()).unwrap();
    db.set_key_constraints("N", pk(&["n"])).unwrap();
    db.insert("N", tuple! { n: 1i64 }).unwrap();
    db.insert("N", tuple! { n: 2i64 }).unwrap();
    db
}

/// "N holds at most `max` tuples", as a closure.
fn at_most(max: usize) -> DatabaseAssertion<CatalogEngine> {
    DatabaseAssertion::new(
        "small_n",
        "N holds at most a few tuples",
        move |db: &mut Db| {
            db.query("N")
                .map(|n| n.cardinality() <= max)
                .unwrap_or(false)
        },
    )
}

/// A database with `small_n` (at most 3) registered, closed and reopened
/// without registering it again.
fn reopened_without_registration() -> Db {
    let mut db = n_db();
    db.add_assertion(at_most(3)).unwrap();
    reopen(db)
}

fn is_unregistered<T>(result: &Result<T, DatabaseError>) -> bool {
    matches!(result, Err(DatabaseError::UnregisteredAssertion(name)) if name == "small_n")
}

fn ns(db: &Db) -> Vec<i64> {
    let mut values: Vec<i64> = db
        .query("N")
        .unwrap()
        .tuples()
        .filter_map(|t| t.get_typed::<i64>("n"))
        .collect();
    values.sort_unstable();
    values
}

#[test]
fn closure_assertion_names_are_stored_in_the_catalog() {
    let mut db = n_db();
    db.add_assertion(at_most(3)).unwrap();

    let stored = db.engine.stored.clone().unwrap();
    assert_eq!(stored.database().closure_assertions, vec!["small_n"]);

    assert!(db.remove_assertion("small_n").unwrap());
    let stored = db.engine.stored.clone().unwrap();
    assert!(stored.database().closure_assertions.is_empty());
}

#[test]
fn reopened_database_lists_unregistered_assertions() {
    let db = reopened_without_registration();
    assert_eq!(db.unregistered_assertions(), vec!["small_n".to_string()]);
    assert!(db.assertions().is_empty());
}

#[test]
fn every_change_fails_until_the_assertion_is_registered() {
    let mut db = reopened_without_registration();

    assert!(is_unregistered(&db.insert("N", tuple! { n: 3i64 })));
    assert!(is_unregistered(
        &db.bulk_insert("N", vec![tuple! { n: 3i64 }])
    ));
    assert!(is_unregistered(&db.delete("N", |_| true)));
    assert!(is_unregistered(&db.update("N", |_| true, |t| t.clone())));
    assert!(is_unregistered(
        &db.multiple_assign(vec![("N", Relation::new(n_type()))])
    ));
    assert!(is_unregistered(&db.create_relvar("M", n_type())));
    assert!(is_unregistered(&db.drop_relvar("N")));
    assert!(is_unregistered(&db.set_key_constraints("N", pk(&["n"]))));
    assert!(is_unregistered(
        &db.set_foreign_key_constraints("N", ForeignKeyConstraints::new())
    ));
    assert!(is_unregistered(&db.set_type_constraints(
        "N",
        "n",
        AttributeConstraints::new("n".to_string(), ScalarType::Int).with_constraint(
            TypeConstraint::Range {
                min: ScalarValue::Int(0),
                max: ScalarValue::Int(9),
            }
        ),
    )));
    assert!(is_unregistered(
        &db.set_check_constraints("N", CheckConstraints::new())
    ));

    assert_eq!(ns(&db), vec![1, 2]);
    assert!(db.relvar_exists("N") && !db.relvar_exists("M"));
}

#[test]
fn changes_inside_a_transaction_fail_too() {
    let mut db = reopened_without_registration();
    db.begin().unwrap();
    assert!(is_unregistered(&db.insert("N", tuple! { n: 3i64 })));
    db.rollback().unwrap();
    assert_eq!(ns(&db), vec![1, 2]);
}

#[test]
fn queries_keep_working() {
    let db = reopened_without_registration();
    assert_eq!(ns(&db), vec![1, 2]);
    let projected = Query::scan("N").project(vec!["n"]).execute(&db).unwrap();
    assert_eq!(projected.cardinality(), 2);
}

#[test]
fn verify_integrity_fails_while_an_assertion_is_unregistered() {
    let mut db = reopened_without_registration();
    assert!(is_unregistered(&db.verify_integrity()));
}

#[test]
fn re_registering_restores_changes_and_enforcement() {
    let mut db = reopened_without_registration();
    db.add_assertion(at_most(3)).unwrap();

    assert!(db.unregistered_assertions().is_empty());
    db.insert("N", tuple! { n: 3i64 }).unwrap();
    assert!(matches!(
        db.insert("N", tuple! { n: 4i64 }),
        Err(DatabaseError::AssertionViolation(_))
    ));
    assert!(db.verify_integrity().is_ok());
    // Re-registering does not record the name a second time.
    let stored = db.engine.stored.clone().unwrap();
    assert_eq!(stored.database().closure_assertions, vec!["small_n"]);
}

#[test]
fn re_registration_is_validated_against_the_current_data() {
    let mut db = reopened_without_registration();
    // N holds two tuples; a rule allowing only one does not hold.
    let result = db.add_assertion(at_most(1));
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(db.unregistered_assertions(), vec!["small_n".to_string()]);
    assert!(is_unregistered(&db.insert("N", tuple! { n: 3i64 })));
}

#[test]
fn re_registration_inside_a_transaction_is_rejected() {
    let mut db = reopened_without_registration();
    db.begin().unwrap();
    assert!(matches!(
        db.add_assertion(at_most(3)),
        Err(DatabaseError::TransactionError(_))
    ));
    db.rollback().unwrap();
    assert_eq!(db.unregistered_assertions().len(), 1);
}

#[test]
fn forgetting_an_assertion_drops_it_for_good() {
    let mut db = reopened_without_registration();
    assert!(db.forget_assertion("small_n").unwrap());

    assert!(db.unregistered_assertions().is_empty());
    db.insert("N", tuple! { n: 3i64 }).unwrap();
    db.insert("N", tuple! { n: 4i64 }).unwrap();
    assert!(db.verify_integrity().is_ok());

    let db = reopen(db);
    assert!(db.unregistered_assertions().is_empty());
}

#[test]
fn forget_only_drops_unregistered_names() {
    let mut db = n_db();
    db.add_assertion(at_most(3)).unwrap();
    // A registered assertion is removed with remove_assertion instead.
    assert!(!db.forget_assertion("small_n").unwrap());
    assert!(!db.forget_assertion("no_such_assertion").unwrap());
    assert_eq!(db.assertions().len(), 1);
}

#[test]
fn forgetting_inside_a_transaction_is_rejected() {
    let mut db = reopened_without_registration();
    db.begin().unwrap();
    assert!(matches!(
        db.forget_assertion("small_n"),
        Err(DatabaseError::TransactionError(_))
    ));
}

#[test]
fn removing_a_closure_assertion_forgets_its_name() {
    let mut db = n_db();
    db.add_assertion(at_most(3)).unwrap();
    assert!(db.remove_assertion("small_n").unwrap());

    let mut db = reopen(db);
    assert!(db.unregistered_assertions().is_empty());
    db.insert("N", tuple! { n: 3i64 }).unwrap();
}

#[test]
fn an_unregistered_name_is_still_taken() {
    let mut db = reopened_without_registration();
    let result = db.add_declarative_assertion(DeclarativeAssertion::new(
        "small_n",
        "Clashes with the unregistered closure assertion",
        AssertionCondition::NotEmpty(Query::scan("N")),
    ));
    assert!(matches!(
        result,
        Err(DatabaseError::Assertion(AssertionError::DuplicateName(name))) if name == "small_n"
    ));
}

#[test]
fn declarative_assertions_can_be_added_while_incomplete() {
    // Adding a constraint changes no data; it is checked on its own.
    let mut db = reopened_without_registration();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "n_not_empty",
        "N has a tuple",
        AssertionCondition::NotEmpty(Query::scan("N")),
    ))
    .unwrap();
    assert!(is_unregistered(&db.insert("N", tuple! { n: 3i64 })));
}

#[test]
fn failed_store_leaves_a_new_closure_assertion_unregistered() {
    let mut db = n_db();
    db.engine.fail_store = true;
    assert!(matches!(
        db.add_assertion(at_most(3)),
        Err(DatabaseError::Storage(_))
    ));
    assert!(db.assertions().is_empty());
}

#[test]
fn several_unregistered_assertions_are_all_listed() {
    let mut db = n_db();
    db.add_assertion(at_most(3)).unwrap();
    db.add_assertion(DatabaseAssertion::new(
        "always",
        "Always holds",
        |_: &mut Db| true,
    ))
    .unwrap();
    let mut db = reopen(db);

    let mut names: Vec<String> = db.unregistered_assertions();
    names.sort();
    assert_eq!(names, vec!["always".to_string(), "small_n".to_string()]);
    db.add_assertion(at_most(3)).unwrap();
    assert!(matches!(
        db.insert("N", tuple! { n: 3i64 }),
        Err(DatabaseError::UnregisteredAssertion(name)) if name == "always"
    ));
}
