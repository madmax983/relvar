//! TTM Golden Rule regression tests (RM Prescription 23).
//!
//! Every database constraint must be satisfied at every statement boundary.
//! These tests drive the database through statements and then ask
//! [`Database::verify_integrity`] whether the resulting state still
//! satisfies every declared constraint.

use crate::constraints::{
    AttributeConstraints, CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression,
    ConstraintManagerError, DatabaseAssertion, ForeignKey, ForeignKeyConstraints, KeyConstraints,
    PrimaryKey, TypeConstraint, ValueOrRef,
};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::{InMemoryEngine, StorageEngine};
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::ScalarValue;
use alloc::string::ToString;

fn id_name_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

fn pk(attributes: &[&str]) -> KeyConstraints {
    KeyConstraints::new().with_primary_key(
        PrimaryKey::new(attributes.iter().map(|a| a.to_string()).collect()).unwrap(),
    )
}

fn fk(from: &[&str], to_relvar: &str, to: &[&str]) -> ForeignKeyConstraints {
    ForeignKeyConstraints::new().with_foreign_key(
        ForeignKey::new(
            from.iter().map(|a| a.to_string()).collect(),
            to_relvar.to_string(),
            to.iter().map(|a| a.to_string()).collect(),
        )
        .unwrap(),
    )
}

/// DEPT{dept_id} with key {dept_id}; EMP{emp_id, dept_id} with key {emp_id}
/// and FK dept_id -> DEPT.dept_id.
fn dept_emp_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "DEPT",
        RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int)),
    )
    .unwrap();
    db.create_relvar(
        "EMP",
        RelationType::new(
            TupleType::new()
                .with_attribute("emp_id", ScalarType::Int)
                .with_attribute("dept_id", ScalarType::Int),
        ),
    )
    .unwrap();
    db.set_key_constraints("DEPT", pk(&["dept_id"])).unwrap();
    db.set_key_constraints("EMP", pk(&["emp_id"])).unwrap();
    db.set_foreign_key_constraints("EMP", fk(&["dept_id"], "DEPT", &["dept_id"]))
        .unwrap();
    db
}

// --- Finding 7: verify_integrity -------------------------------------------

#[test]
fn verify_integrity_accepts_consistent_database() {
    let mut db = dept_emp_db();
    db.insert("DEPT", tuple! { dept_id: 10i64 }).unwrap();
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64 })
        .unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn verify_integrity_detects_key_violation_written_behind_the_database() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.set_key_constraints("R", pk(&["id"])).unwrap();
    // Bypass the Database layer: write straight to the engine.
    db.engine
        .insert_tuple("R", tuple! { id: 1i64, name: "a" })
        .unwrap();
    db.engine
        .insert_tuple("R", tuple! { id: 1i64, name: "b" })
        .unwrap();
    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn verify_integrity_detects_foreign_key_violation() {
    let mut db = dept_emp_db();
    db.engine
        .insert_tuple("EMP", tuple! { emp_id: 1i64, dept_id: 99i64 })
        .unwrap();
    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn verify_integrity_detects_check_constraint_violation() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.set_check_constraints(
        "R",
        CheckConstraints::new().with_constraint(CheckConstraint::new(
            "positive_id",
            "id must be positive",
            ConstraintExpression::Cmp {
                left: "id".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        )),
    )
    .unwrap();
    db.engine
        .insert_tuple("R", tuple! { id: -1i64, name: "a" })
        .unwrap();
    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn verify_integrity_detects_type_constraint_violation() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.set_type_constraints(
        "R",
        "id",
        AttributeConstraints::new("id".to_string(), ScalarType::Int).with_constraint(
            TypeConstraint::Range {
                min: ScalarValue::Int(0),
                max: ScalarValue::Int(9),
            },
        ),
    )
    .unwrap();
    db.engine
        .insert_tuple("R", tuple! { id: 10i64, name: "a" })
        .unwrap();
    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TypeConstraintViolation(_)
        ))
    ));
}

#[test]
fn verify_integrity_detects_assertion_violation() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.add_assertion(DatabaseAssertion::new(
        "at_most_one",
        "R holds at most one tuple",
        |db: &mut Database<InMemoryEngine>| {
            db.query("R").map(|r| r.cardinality() <= 1).unwrap_or(false)
        },
    ))
    .unwrap();
    db.engine
        .insert_tuple("R", tuple! { id: 1i64, name: "a" })
        .unwrap();
    db.engine
        .insert_tuple("R", tuple! { id: 2i64, name: "b" })
        .unwrap();
    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::AssertionViolation(_))
    ));
}

#[test]
fn verify_integrity_detects_tuple_not_conforming_to_heading() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    // Smuggle a non-conforming tuple past `Relation`'s own heading check.
    let mut body = crate::collections::HashSet::new();
    body.insert(tuple! { id: "not an int", name: "a" });
    let bad = crate::values::Relation::from_body_unchecked(id_name_type(), body);
    db.engine.store_relation("R", &bad).unwrap();
    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TupleMismatch
        ))
    ));
}
