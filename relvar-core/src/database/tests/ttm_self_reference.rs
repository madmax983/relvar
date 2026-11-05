//! Self-referencing foreign key regression tests (Golden Rule, RM
//! Prescription 23).
//!
//! A statement must be validated against the *candidate* post-statement
//! state: for a foreign key from a relvar to itself, both the referencing
//! and the referenced side are the relvar's new value.

use super::golden_rule::{fk, pk};
use crate::constraints::ConstraintManagerError;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Tuple;
use alloc::vec;

/// R{id, parent} with key {id} and foreign key parent -> R.id.
fn tree_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "R",
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("parent", ScalarType::Int),
        ),
    )
    .unwrap();
    db.set_key_constraints("R", pk(&["id"])).unwrap();
    db.set_foreign_key_constraints("R", fk(&["parent"], "R", &["id"]))
        .unwrap();
    db
}

fn node(id: i64, parent: i64) -> Tuple {
    tuple! { id: id, parent: parent }
}

fn id_of(t: &Tuple) -> i64 {
    t.get_typed::<i64>("id").unwrap_or_default()
}

fn is_fk_violation<T: core::fmt::Debug>(result: &Result<T, DatabaseError>) -> bool {
    matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    )
}

#[test]
fn insert_of_self_referencing_root_is_accepted() {
    // {(1,1)} satisfies parent -> id: the tuple references itself.
    let mut db = tree_db();
    db.insert("R", node(1, 1)).unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn bulk_insert_satisfying_self_reference_within_batch_is_accepted() {
    let mut db = tree_db();
    db.bulk_insert("R", vec![node(3, 2), node(2, 1), node(1, 1)])
        .unwrap();
    assert_eq!(db.query("R").unwrap().cardinality(), 3);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn bulk_insert_with_dangling_self_reference_is_rejected() {
    let mut db = tree_db();
    db.insert("R", node(1, 1)).unwrap();
    let result = db.bulk_insert("R", vec![node(2, 1), node(4, 3)]);
    assert!(is_fk_violation(&result));
    assert_eq!(db.query("R").unwrap().cardinality(), 1);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn update_creating_dangling_self_reference_is_rejected() {
    // {(1,1),(5,1)}: replacing (5,1) by (6,5) yields {(1,1),(6,5)}, where
    // 6 references 5 -- which no longer exists.
    let mut db = tree_db();
    db.bulk_insert("R", vec![node(1, 1), node(5, 1)]).unwrap();
    let result = db.update("R", |t| id_of(t) == 5, |_| node(6, 5));
    assert!(is_fk_violation(&result));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn update_orphaning_children_is_rejected() {
    // Renaming the root 1 -> 10 (and its self-reference) orphans (2,1).
    let mut db = tree_db();
    db.bulk_insert("R", vec![node(1, 1), node(2, 1)]).unwrap();
    let result = db.update("R", |t| id_of(t) == 1, |_| node(10, 10));
    assert!(is_fk_violation(&result));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn update_renaming_parent_and_children_together_is_accepted() {
    let mut db = tree_db();
    db.bulk_insert("R", vec![node(1, 1), node(2, 1)]).unwrap();
    // Shift every id and parent by 10: the new state is consistent.
    db.update(
        "R",
        |_| true,
        |t| {
            node(
                id_of(t) + 10,
                t.get_typed::<i64>("parent").unwrap_or_default() + 10,
            )
        },
    )
    .unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn delete_of_self_referencing_tuple_is_accepted() {
    // Deleting (1,1) from {(1,1)} leaves {}, which satisfies the FK.
    let mut db = tree_db();
    db.insert("R", node(1, 1)).unwrap();
    assert_eq!(db.delete("R", |t| id_of(t) == 1).unwrap(), 1);
    assert!(db.query("R").unwrap().is_empty());
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn delete_of_whole_subtree_is_accepted() {
    let mut db = tree_db();
    db.bulk_insert("R", vec![node(1, 1), node(2, 1), node(3, 2)])
        .unwrap();
    assert_eq!(db.delete("R", |t| id_of(t) >= 2).unwrap(), 2);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn delete_orphaning_a_child_is_rejected() {
    let mut db = tree_db();
    db.bulk_insert("R", vec![node(1, 1), node(2, 1)]).unwrap();
    let result = db.delete("R", |t| id_of(t) == 1);
    assert!(is_fk_violation(&result));
    assert_eq!(db.query("R").unwrap().cardinality(), 2);
}

#[test]
fn update_of_referencing_relvar_checks_referenced_relvar() {
    // Non-self foreign keys keep working on update.
    let mut db = super::golden_rule::dept_emp_db();
    db.insert("DEPT", tuple! { dept_id: 10i64 }).unwrap();
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64 })
        .unwrap();
    let result = db.update("EMP", |_| true, |_| tuple! { emp_id: 1i64, dept_id: 99i64 });
    assert!(is_fk_violation(&result));
    assert!(db.verify_integrity().is_ok());
}
