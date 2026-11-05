//! Foreign key declaration regression tests.
//!
//! TTM: a foreign key references a candidate key of the referenced relvar,
//! and each referencing attribute has the same type as the attribute it
//! references.

use super::golden_rule::{fk, pk};
use super::ttm_keys::keys;
use crate::constraints::ConstraintManagerError;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::types::{RelationType, ScalarType, TupleType};

/// DEPT{dept_id: Int, site: Int, name: String} (no keys yet) and
/// EMP{emp_id: Int, dept_id: Int, site: Int, dept_name: String, code: String}.
fn unkeyed_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "DEPT",
        RelationType::new(
            TupleType::new()
                .with_attribute("dept_id", ScalarType::Int)
                .with_attribute("site", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        ),
    )
    .unwrap();
    db.create_relvar(
        "EMP",
        RelationType::new(
            TupleType::new()
                .with_attribute("emp_id", ScalarType::Int)
                .with_attribute("dept_id", ScalarType::Int)
                .with_attribute("site", ScalarType::Int)
                .with_attribute("dept_name", ScalarType::String)
                .with_attribute("code", ScalarType::String),
        ),
    )
    .unwrap();
    db
}

fn is_target_not_key(result: &Result<(), DatabaseError>) -> bool {
    matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyTargetNotKey { .. }
        ))
    )
}

#[test]
fn foreign_key_to_relvar_without_keys_is_rejected() {
    let mut db = unkeyed_db();
    let result = db.set_foreign_key_constraints("EMP", fk(&["dept_id"], "DEPT", &["dept_id"]));
    assert!(is_target_not_key(&result));
    assert!(db.get_foreign_key_constraints("EMP").is_none());
}

#[test]
fn foreign_key_to_non_key_attribute_is_rejected() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id"])).unwrap();
    let result = db.set_foreign_key_constraints("EMP", fk(&["dept_name"], "DEPT", &["name"]));
    assert!(is_target_not_key(&result));
}

#[test]
fn foreign_key_to_part_of_composite_key_is_rejected() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id", "site"]))
        .unwrap();
    let result = db.set_foreign_key_constraints("EMP", fk(&["dept_id"], "DEPT", &["dept_id"]));
    assert!(is_target_not_key(&result));
}

#[test]
fn foreign_key_to_composite_key_in_any_order_is_accepted() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id", "site"]))
        .unwrap();
    // Keys are attribute sets: the reference may list them in any order,
    // as long as each referencing attribute lines up with its target.
    assert!(
        db.set_foreign_key_constraints(
            "EMP",
            fk(&["site", "dept_id"], "DEPT", &["site", "dept_id"])
        )
        .is_ok()
    );
}

#[test]
fn foreign_key_to_candidate_key_is_accepted() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", keys(Some(&["dept_id"]), &[&["name"]]))
        .unwrap();
    assert!(
        db.set_foreign_key_constraints("EMP", fk(&["dept_name"], "DEPT", &["name"]))
            .is_ok()
    );
}

#[test]
fn foreign_key_with_mismatched_types_is_rejected() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id"])).unwrap();
    // EMP.code is a String, DEPT.dept_id an Int.
    let result = db.set_foreign_key_constraints("EMP", fk(&["code"], "DEPT", &["dept_id"]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyTypeMismatch { .. }
        ))
    ));
}

#[test]
fn foreign_key_with_unknown_referencing_attribute_is_rejected() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id"])).unwrap();
    let result = db.set_foreign_key_constraints("EMP", fk(&["ghost"], "DEPT", &["dept_id"]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::AttributeNotFound(attr, rel)
        )) if attr == "ghost" && rel == "EMP"
    ));
}

#[test]
fn foreign_key_to_missing_relvar_is_rejected() {
    let mut db = unkeyed_db();
    let result = db.set_foreign_key_constraints("EMP", fk(&["dept_id"], "NOPE", &["dept_id"]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::RelationNotFound(name)
        )) if name == "NOPE"
    ));
}

#[test]
fn foreign_key_repeating_an_attribute_is_rejected() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id", "site"]))
        .unwrap();
    let result = db.set_foreign_key_constraints(
        "EMP",
        fk(&["dept_id", "dept_id"], "DEPT", &["dept_id", "site"]),
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::DuplicateKeyAttribute { .. }
        ))
    ));
}

#[test]
fn replacing_keys_cannot_drop_a_referenced_key() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id"])).unwrap();
    db.set_foreign_key_constraints("EMP", fk(&["dept_id"], "DEPT", &["dept_id"]))
        .unwrap();

    let result = db.set_key_constraints("DEPT", pk(&["name"]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::KeyReferencedByForeignKey { ref referencing, .. }
        )) if referencing == "EMP"
    ));
    // The original key is still in force.
    let current = db.get_key_constraints("DEPT").unwrap();
    assert_eq!(current.primary_key().unwrap().attributes(), &["dept_id"]);
}

#[test]
fn replacing_keys_may_keep_the_referenced_key_as_a_candidate_key() {
    let mut db = unkeyed_db();
    db.set_key_constraints("DEPT", pk(&["dept_id"])).unwrap();
    db.set_foreign_key_constraints("EMP", fk(&["dept_id"], "DEPT", &["dept_id"]))
        .unwrap();
    assert!(
        db.set_key_constraints("DEPT", keys(Some(&["name"]), &[&["dept_id"]]))
            .is_ok()
    );
}
