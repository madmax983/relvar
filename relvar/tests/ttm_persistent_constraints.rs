//! A database contains its constraints (TTM RM Prescription 16): keys,
//! foreign keys, CHECK constraints and type (attribute) constraints declared
//! on a persistent database must survive closing and reopening it, and keep
//! being enforced afterwards.

#![cfg(feature = "storage")]

use relvar::constraints::{
    AttributeConstraints, CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression,
    ConstraintManagerError, ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey,
    TypeConstraint, ValueOrRef,
};
use relvar::{
    Database, DatabaseError, PersistentEngine, RelationType, ScalarType, ScalarValue, TupleType,
    tuple,
};
use std::path::Path;

type Db = Database<PersistentEngine>;

fn open(dir: &Path) -> Db {
    Database::new(PersistentEngine::open(dir).unwrap())
}

fn dept_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("budget", ScalarType::Int),
    )
}

fn emp_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    )
}

fn pk(attribute: &str) -> KeyConstraints {
    KeyConstraints::new().with_primary_key(PrimaryKey::new(vec![attribute.to_string()]).unwrap())
}

/// DEPT(dept_id key, budget >= 0 CHECK) and EMP(emp_id key, dept_id -> DEPT,
/// age in 16..=99 type constraint), with one department and one employee.
fn build_constrained_db(dir: &Path) {
    let mut db = open(dir);
    db.create_relvar("DEPT", dept_type()).unwrap();
    db.create_relvar("EMP", emp_type()).unwrap();
    db.set_key_constraints("DEPT", pk("dept_id")).unwrap();
    db.set_key_constraints("EMP", pk("emp_id")).unwrap();
    db.set_foreign_key_constraints(
        "EMP",
        ForeignKeyConstraints::new().with_foreign_key(
            ForeignKey::new(
                vec!["dept_id".to_string()],
                "DEPT".to_string(),
                vec!["dept_id".to_string()],
            )
            .unwrap(),
        ),
    )
    .unwrap();
    db.set_check_constraints(
        "DEPT",
        CheckConstraints::new().with_constraint(CheckConstraint::new(
            "non_negative_budget",
            "A budget is never negative",
            ConstraintExpression::Cmp {
                left: "budget".to_string(),
                op: CmpOp::Ge,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        )),
    )
    .unwrap();
    db.set_type_constraints(
        "EMP",
        "age",
        AttributeConstraints::new("age".to_string(), ScalarType::Int).with_constraint(
            TypeConstraint::Range {
                min: ScalarValue::Int(16),
                max: ScalarValue::Int(99),
            },
        ),
    )
    .unwrap();
    db.insert("DEPT", tuple! { dept_id: 10i64, budget: 100i64 })
        .unwrap();
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64, age: 30i64 })
        .unwrap();
}

#[test]
fn key_constraints_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    let mut db = open(dir.path());

    assert!(db.get_key_constraints("DEPT").is_some());
    let duplicate = db.insert("DEPT", tuple! { dept_id: 10i64, budget: 5i64 });
    assert!(matches!(
        duplicate,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn foreign_keys_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    let mut db = open(dir.path());

    assert!(db.get_foreign_key_constraints("EMP").is_some());
    let orphan = db.insert("EMP", tuple! { emp_id: 2i64, dept_id: 99i64, age: 30i64 });
    assert!(matches!(
        orphan,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    // The reverse direction is enforced too: DEPT 10 is still referenced.
    let orphaning = db.delete("DEPT", |_| true);
    assert!(orphaning.is_err());
    // ... and the referenced relvar still cannot be dropped.
    assert!(db.drop_relvar("DEPT").is_err());
}

#[test]
fn check_constraints_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    let mut db = open(dir.path());

    let negative = db.insert("DEPT", tuple! { dept_id: 20i64, budget: -1i64 });
    assert!(matches!(
        negative,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn type_constraints_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    let mut db = open(dir.path());

    let too_young = db.insert("EMP", tuple! { emp_id: 2i64, dept_id: 10i64, age: 3i64 });
    assert!(matches!(
        too_young,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TypeConstraintViolation(_)
        ))
    ));
}

#[test]
fn reopened_database_satisfies_verify_integrity() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    let mut db = open(dir.path());

    assert!(db.verify_integrity().is_ok());
    // Valid writes still go through after reopening.
    db.insert("DEPT", tuple! { dept_id: 20i64, budget: 0i64 })
        .unwrap();
    db.insert("EMP", tuple! { emp_id: 2i64, dept_id: 20i64, age: 16i64 })
        .unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn verify_integrity_detects_data_violating_reopened_constraints() {
    use relvar::StorageEngine;

    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    {
        // Bypass the Database layer: write a duplicate key straight into
        // the engine, as an external tool might.
        let mut engine = PersistentEngine::open(dir.path()).unwrap();
        engine
            .insert_tuple("DEPT", tuple! { dept_id: 10i64, budget: 7i64 })
            .unwrap();
    }
    let mut db = open(dir.path());

    assert!(matches!(
        db.verify_integrity(),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn replaced_constraints_are_persisted() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    {
        let mut db = open(dir.path());
        // Replace the CHECK constraints with none: negative budgets allowed.
        db.set_check_constraints("DEPT", CheckConstraints::new())
            .unwrap();
    }
    let mut db = open(dir.path());
    db.insert("DEPT", tuple! { dept_id: 20i64, budget: -1i64 })
        .unwrap();
}

#[test]
fn dropped_relvar_constraints_do_not_reappear() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    {
        let mut db = open(dir.path());
        db.drop_relvar("EMP").unwrap();
        db.create_relvar("EMP", emp_type()).unwrap();
    }
    let mut db = open(dir.path());

    assert!(db.get_key_constraints("EMP").is_none());
    assert!(db.get_foreign_key_constraints("EMP").is_none());
    // No key, no foreign key, no type constraint on the recreated relvar.
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 99i64, age: 3i64 })
        .unwrap();
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 98i64, age: 3i64 })
        .unwrap();
    // DEPT's own constraints are untouched.
    assert!(db.get_key_constraints("DEPT").is_some());
}

#[test]
fn catalog_without_constraints_still_opens() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());

    // Rewrite the catalog in the pre-constraint format: relations only.
    let catalog_path = dir.path().join("catalog.json");
    let text = std::fs::read_to_string(&catalog_path).unwrap();
    let mut catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
    let object = catalog.as_object_mut().unwrap();
    object.retain(|field, _| field == "relations");
    std::fs::write(&catalog_path, serde_json::to_string(&catalog).unwrap()).unwrap();

    let mut db = open(dir.path());

    // The data is intact and there are no constraints.
    assert_eq!(db.query("DEPT").unwrap().cardinality(), 1);
    assert!(db.get_key_constraints("DEPT").is_none());
    db.insert("DEPT", tuple! { dept_id: 10i64, budget: -5i64 })
        .unwrap();
}

#[test]
fn relvar_open_restores_constraints() {
    let dir = tempfile::tempdir().unwrap();
    build_constrained_db(dir.path());
    let mut db = relvar::open(dir.path()).unwrap();

    assert!(
        db.insert("DEPT", tuple! { dept_id: 10i64, budget: 1i64 })
            .is_err()
    );
}
