//! Schema-change regression tests: dropping relvars and DDL inside
//! transactions (RM Prescriptions 16 and 23).
//!
//! A database is a container of relvars *and their constraints*; dropping
//! a relvar must neither leave dangling foreign keys behind nor orphan
//! tuples, and DDL (which is not transactional) is rejected inside a
//! transaction.

use super::golden_rule::{dept_emp_db, fk, id_name_type, pk};
use crate::constraints::{
    AttributeConstraints, CheckConstraints, ConstraintManagerError, DatabaseAssertion,
};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};

fn emp_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("dept_id", ScalarType::Int),
    )
}

#[test]
fn dropping_a_referenced_relvar_is_rejected() {
    let mut db = dept_emp_db();
    db.insert("DEPT", tuple! { dept_id: 10i64 }).unwrap();
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64 })
        .unwrap();

    let result = db.drop_relvar("DEPT");
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::RelvarReferenced { ref relation, ref referencing }
        )) if relation == "DEPT" && referencing == "EMP"
    ));
    // Nothing changed: DEPT, its key and EMP's foreign key are intact.
    assert!(db.relvar_exists("DEPT"));
    assert!(db.get_key_constraints("DEPT").is_some());
    assert!(db.get_foreign_key_constraints("EMP").is_some());
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn dropping_referencing_then_referenced_relvar_succeeds() {
    let mut db = dept_emp_db();
    db.drop_relvar("EMP").unwrap();
    db.drop_relvar("DEPT").unwrap();
    assert!(db.get_foreign_key_constraints("EMP").is_none());
    assert!(db.get_key_constraints("DEPT").is_none());
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn recreated_relvar_does_not_inherit_dropped_constraints() {
    let mut db = dept_emp_db();
    db.drop_relvar("EMP").unwrap();
    db.create_relvar("EMP", emp_type()).unwrap();
    // No foreign key and no key survive the drop.
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 99i64 })
        .unwrap();
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 98i64 })
        .unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn dropping_a_relvar_referenced_only_by_itself_succeeds() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "TREE",
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("parent", ScalarType::Int),
        ),
    )
    .unwrap();
    db.set_key_constraints("TREE", pk(&["id"])).unwrap();
    db.set_foreign_key_constraints("TREE", fk(&["parent"], "TREE", &["id"]))
        .unwrap();
    db.drop_relvar("TREE").unwrap();
    assert!(!db.relvar_exists("TREE"));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn dropping_a_missing_relvar_reports_not_found() {
    let mut db = dept_emp_db();
    assert!(db.drop_relvar("NOPE").is_err());
    assert!(db.verify_integrity().is_ok());
}

fn is_transaction_error(result: &Result<(), DatabaseError>) -> bool {
    matches!(result, Err(DatabaseError::TransactionError(_)))
}

#[test]
fn create_relvar_inside_transaction_is_rejected() {
    // Before: the relvar survived the rollback (the in-memory engine only
    // restores relations that existed when the transaction began).
    let mut db = Database::new(InMemoryEngine::new());
    db.begin().unwrap();
    assert!(is_transaction_error(&db.create_relvar("T", id_name_type())));
    db.rollback().unwrap();
    assert!(!db.relvar_exists("T"));
}

#[test]
fn drop_relvar_inside_transaction_is_rejected() {
    // Before: the dropped relvar was not restored by the rollback.
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("T", id_name_type()).unwrap();
    db.insert("T", tuple! { id: 1i64, name: "a" }).unwrap();
    db.begin().unwrap();
    assert!(is_transaction_error(&db.drop_relvar("T")));
    db.rollback().unwrap();
    assert_eq!(db.query("T").unwrap().cardinality(), 1);
}

#[test]
fn constraint_ddl_inside_transaction_is_rejected() {
    let mut db = dept_emp_db();
    db.begin().unwrap();
    assert!(is_transaction_error(
        &db.set_key_constraints("DEPT", pk(&["dept_id"]))
    ));
    assert!(is_transaction_error(&db.set_foreign_key_constraints(
        "EMP",
        fk(&["dept_id"], "DEPT", &["dept_id"])
    )));
    assert!(is_transaction_error(
        &db.set_check_constraints("EMP", CheckConstraints::new())
    ));
    assert!(is_transaction_error(&db.set_type_constraints(
        "EMP",
        "emp_id",
        AttributeConstraints::new("emp_id".into(), ScalarType::Int)
    )));
    assert!(is_transaction_error(&db.add_assertion(
        DatabaseAssertion::new("t", "always", |_: &mut Database<InMemoryEngine>| true)
    )));
    db.commit().unwrap();
    assert!(db.assertions().is_empty());
}
