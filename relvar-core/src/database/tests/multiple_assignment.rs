//! Multiple assignment (TTM RM Prescription 21).
//!
//! Several relvar assignments performed as one statement: the Golden Rule
//! (RM Prescription 23) is checked only on the combined final state, and
//! either every assignment takes effect or none does.

use super::golden_rule::{fk, id_name_type, pk};
use crate::constraints::{ConstraintManagerError, DatabaseAssertion};
use crate::database::{Assignment, Database};
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, Tuple};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

fn a_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("a_id", ScalarType::Int)
            .with_attribute("b_ref", ScalarType::Int),
    )
}

fn b_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("b_id", ScalarType::Int)
            .with_attribute("a_ref", ScalarType::Int),
    )
}

fn a_rel(tuples: Vec<Tuple>) -> Relation {
    Relation::from_tuples(a_type(), tuples).unwrap()
}

fn b_rel(tuples: Vec<Tuple>) -> Relation {
    Relation::from_tuples(b_type(), tuples).unwrap()
}

/// A{a_id, b_ref} and B{b_id, a_ref}, each keyed on its id, referencing
/// each other: A.b_ref -> B.b_id and B.a_ref -> A.a_id. Starting from the
/// empty state, no single-relvar statement can insert a tuple into either.
fn cyclic_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("A", a_type()).unwrap();
    db.create_relvar("B", b_type()).unwrap();
    db.set_key_constraints("A", pk(&["a_id"])).unwrap();
    db.set_key_constraints("B", pk(&["b_id"])).unwrap();
    db.set_foreign_key_constraints("A", fk(&["b_ref"], "B", &["b_id"]))
        .unwrap();
    db.set_foreign_key_constraints("B", fk(&["a_ref"], "A", &["a_id"]))
        .unwrap();
    db
}

fn pair() -> Vec<(&'static str, Relation)> {
    vec![
        ("A", a_rel(vec![tuple! { a_id: 1i64, b_ref: 1i64 }])),
        ("B", b_rel(vec![tuple! { b_id: 1i64, a_ref: 1i64 }])),
    ]
}

fn cardinalities(db: &Database<InMemoryEngine>) -> (usize, usize) {
    (
        db.query("A").unwrap().cardinality(),
        db.query("B").unwrap().cardinality(),
    )
}

#[test]
fn sequential_statements_cannot_populate_cyclic_relvars() {
    let mut db = cyclic_db();
    assert!(db.insert("A", tuple! { a_id: 1i64, b_ref: 1i64 }).is_err());
    assert!(db.insert("B", tuple! { b_id: 1i64, a_ref: 1i64 }).is_err());
}

#[test]
fn multiple_assignment_populates_cyclic_relvars() {
    let mut db = cyclic_db();
    db.multiple_assign(pair()).unwrap();

    assert_eq!(cardinalities(&db), (1, 1));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn multiple_assignment_empties_cyclic_relvars() {
    let mut db = cyclic_db();
    db.multiple_assign(pair()).unwrap();

    db.multiple_assign(vec![("A", a_rel(vec![])), ("B", b_rel(vec![]))])
        .unwrap();
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn foreign_key_violation_in_final_state_changes_nothing() {
    let mut db = cyclic_db();
    let result = db.multiple_assign(vec![
        ("A", a_rel(vec![tuple! { a_id: 1i64, b_ref: 1i64 }])),
        ("B", b_rel(vec![tuple! { b_id: 1i64, a_ref: 2i64 }])),
    ]);

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn foreign_key_into_unassigned_relvar_is_checked() {
    let mut db = cyclic_db();
    let result = db.multiple_assign(vec![("A", a_rel(vec![tuple! { a_id: 1i64, b_ref: 1i64 }]))]);
    assert!(result.is_err());
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn assertion_violation_undoes_every_assignment() {
    let mut db = cyclic_db();
    db.add_assertion(DatabaseAssertion::new(
        "b_empty",
        "B holds no tuples",
        |db: &mut Database<InMemoryEngine>| db.query("B").map(|b| b.is_empty()).unwrap_or(false),
    ))
    .unwrap();

    let result = db.multiple_assign(pair());
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn assertion_violation_inside_transaction_undoes_only_the_statement() {
    let mut db = cyclic_db();
    db.add_assertion(DatabaseAssertion::new(
        "at_most_one_a",
        "A holds at most one tuple",
        |db: &mut Database<InMemoryEngine>| {
            db.query("A").map(|a| a.cardinality() <= 1).unwrap_or(false)
        },
    ))
    .unwrap();

    db.begin().unwrap();
    db.multiple_assign(pair()).unwrap();
    let violating = db.multiple_assign(vec![
        (
            "A",
            a_rel(vec![
                tuple! { a_id: 1i64, b_ref: 1i64 },
                tuple! { a_id: 2i64, b_ref: 1i64 },
            ]),
        ),
        ("B", b_rel(vec![tuple! { b_id: 1i64, a_ref: 2i64 }])),
    ]);
    assert!(matches!(
        violating,
        Err(DatabaseError::AssertionViolation(_))
    ));
    assert_eq!(cardinalities(&db), (1, 1));
    db.commit().unwrap();

    assert_eq!(cardinalities(&db), (1, 1));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn key_violation_is_rejected() {
    let mut db = cyclic_db();
    let result = db.multiple_assign(vec![
        (
            "A",
            a_rel(vec![
                tuple! { a_id: 1i64, b_ref: 1i64 },
                tuple! { a_id: 1i64, b_ref: 2i64 },
            ]),
        ),
        (
            "B",
            b_rel(vec![
                tuple! { b_id: 1i64, a_ref: 1i64 },
                tuple! { b_id: 2i64, a_ref: 1i64 },
            ]),
        ),
    ]);
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn key_index_reflects_assigned_values() {
    let mut db = cyclic_db();
    db.multiple_assign(pair()).unwrap();

    // a_id 1 is now taken: a later insert must see it.
    let duplicate = db.insert("A", tuple! { a_id: 1i64, b_ref: 1i64 });
    assert!(matches!(
        duplicate,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
    db.insert("A", tuple! { a_id: 2i64, b_ref: 1i64 }).unwrap();
}

#[test]
fn heading_mismatch_is_rejected() {
    let mut db = cyclic_db();
    let wrong =
        Relation::from_tuples(id_name_type(), vec![tuple! { id: 1i64, name: "x" }]).unwrap();
    let result = db.multiple_assign(vec![pair().remove(1), ("A", wrong)]);

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TupleMismatch
        ))
    ));
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn unknown_relvar_is_rejected() {
    let mut db = cyclic_db();
    let mut assignments = pair();
    assignments.push(("MISSING", a_rel(vec![])));

    assert!(matches!(
        db.multiple_assign(assignments),
        Err(DatabaseError::RelationNotFound(name)) if name == "MISSING"
    ));
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn virtual_relvar_target_is_rejected() {
    let mut db = cyclic_db();
    db.define_virtual_relvar("V", a_type(), |_| Ok(Relation::new(a_type())))
        .unwrap();

    assert!(matches!(
        db.multiple_assign(vec![("V", a_rel(vec![]))]),
        Err(DatabaseError::CannotModifyVirtualRelvar(name)) if name == "V"
    ));
}

#[test]
fn assignments_to_the_same_target_are_combined() {
    // RM Prescription 21: `A := a1, A := {}` assigns A the empty relation
    // (the later assignment wins), so B's reference into A dangles in the
    // final state and the whole statement is rejected.
    let mut db = cyclic_db();
    let mut assignments = pair();
    assignments.push(("A", a_rel(vec![])));

    assert!(matches!(
        db.multiple_assign(assignments),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    assert_eq!(cardinalities(&db), (0, 0));
}

// --- Several assignments to one target (RM Prescription 21) ---------------

/// R{id, name} keyed on id, holding (1, "a") and (2, "b"); S{id, name}
/// empty and unconstrained.
fn keyed_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.create_relvar("S", id_name_type()).unwrap();
    db.set_key_constraints("R", pk(&["id"])).unwrap();
    db.insert("R", tuple! { id: 1i64, name: "a" }).unwrap();
    db.insert("R", tuple! { id: 2i64, name: "b" }).unwrap();
    db
}

fn r(tuples: &[(i64, &str)]) -> Relation {
    Relation::from_tuples(
        id_name_type(),
        tuples
            .iter()
            .map(|(id, name)| tuple! { id: *id, name: *name }),
    )
    .unwrap()
}

fn contents(db: &Database<InMemoryEngine>, relvar: &str) -> Vec<(i64, String)> {
    let mut tuples: Vec<(i64, String)> = db
        .query(relvar)
        .unwrap()
        .tuples()
        .filter_map(|t| Some((t.get_typed::<i64>("id")?, t.get_typed::<String>("name")?)))
        .collect();
    tuples.sort();
    tuples
}

fn owned(tuples: &[(i64, &str)]) -> Vec<(i64, String)> {
    tuples
        .iter()
        .map(|(id, name)| (*id, name.to_string()))
        .collect()
}

#[test]
fn insert_then_insert_unions_both_sources() {
    let mut db = keyed_db();
    db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(3, "c")]))),
        ("R", Assignment::Insert(r(&[(4, "d")]))),
    ])
    .unwrap();
    assert_eq!(
        contents(&db, "R"),
        owned(&[(1, "a"), (2, "b"), (3, "c"), (4, "d")])
    );
}

#[test]
fn insert_then_delete_of_the_same_tuple_removes_it() {
    let mut db = keyed_db();
    db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(3, "c")]))),
        ("R", Assignment::Delete(r(&[(3, "c"), (1, "a")]))),
    ])
    .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(2, "b")]));
}

#[test]
fn delete_then_insert_of_the_same_tuple_keeps_it() {
    let mut db = keyed_db();
    db.multiple_assign(vec![
        ("R", Assignment::Delete(r(&[(1, "a")]))),
        ("R", Assignment::Insert(r(&[(1, "a")]))),
    ])
    .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
}

#[test]
fn replace_then_insert_starts_from_the_replacement() {
    let mut db = keyed_db();
    db.multiple_assign(vec![
        ("R", Assignment::Replace(r(&[(5, "e")]))),
        ("R", Assignment::Insert(r(&[(6, "f")]))),
    ])
    .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(5, "e"), (6, "f")]));
}

#[test]
fn replace_then_replace_keeps_the_last() {
    let mut db = keyed_db();
    db.multiple_assign(vec![("R", r(&[(5, "e")])), ("R", r(&[(6, "f")]))])
        .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(6, "f")]));
}

#[test]
fn inserting_present_tuples_changes_nothing() {
    let mut db = keyed_db();
    db.multiple_assign(vec![("R", Assignment::Insert(r(&[(1, "a")])))])
        .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
}

#[test]
fn repeated_targets_mix_with_other_targets() {
    // Cyclic A/B: both relvars get two inserts each, interleaved.
    let mut db = cyclic_db();
    db.multiple_assign(vec![
        (
            "A",
            Assignment::Insert(a_rel(vec![tuple! { a_id: 1i64, b_ref: 1i64 }])),
        ),
        (
            "B",
            Assignment::Insert(b_rel(vec![tuple! { b_id: 1i64, a_ref: 1i64 }])),
        ),
        (
            "A",
            Assignment::Insert(a_rel(vec![tuple! { a_id: 2i64, b_ref: 2i64 }])),
        ),
        (
            "B",
            Assignment::Insert(b_rel(vec![tuple! { b_id: 2i64, a_ref: 2i64 }])),
        ),
    ])
    .unwrap();
    assert_eq!(cardinalities(&db), (2, 2));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn heading_mismatch_in_any_operation_rejects_the_whole_statement() {
    let wrong = || a_rel(vec![tuple! { a_id: 1i64, b_ref: 1i64 }]);
    for bad in [
        Assignment::Replace(wrong()),
        Assignment::Insert(wrong()),
        Assignment::Delete(wrong()),
    ] {
        let mut db = keyed_db();
        let result = db.multiple_assign(vec![
            ("R", Assignment::Insert(r(&[(3, "c")]))),
            ("S", Assignment::Insert(r(&[(9, "z")]))),
            ("R", bad),
        ]);
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::TupleMismatch
            ))
        ));
        assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
        assert!(contents(&db, "S").is_empty());
    }
}

#[test]
fn unknown_target_in_a_later_operation_rejects_the_statement() {
    let mut db = keyed_db();
    let result = db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(3, "c")]))),
        ("MISSING", Assignment::Insert(r(&[(3, "c")]))),
    ]);
    assert!(matches!(result, Err(DatabaseError::RelationNotFound(name)) if name == "MISSING"));
    assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
}

#[test]
fn keys_are_checked_on_the_combined_value() {
    // Two inserts that each keep the key but together break it.
    let mut db = keyed_db();
    let result = db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(3, "c")]))),
        ("R", Assignment::Insert(r(&[(3, "other")]))),
    ]);
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
    assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
}

#[test]
fn intermediate_states_are_not_checked() {
    // Inserting (1, "z") first would break the key on its own; deleting
    // (1, "a") in the same statement leaves a combined value that keeps it.
    let mut db = keyed_db();
    db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(1, "z")]))),
        ("R", Assignment::Delete(r(&[(1, "a")]))),
    ])
    .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(1, "z"), (2, "b")]));

    // The key index follows the combined value: id 1 is taken, by "z".
    assert!(matches!(
        db.insert("R", tuple! { id: 1i64, name: "a" }),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
    db.insert("R", tuple! { id: 3i64, name: "c" }).unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn combined_assignment_inside_a_transaction_is_rolled_back() {
    let mut db = keyed_db();
    db.begin().unwrap();
    db.multiple_assign(vec![
        ("R", Assignment::Delete(r(&[(1, "a")]))),
        ("R", Assignment::Insert(r(&[(7, "g")]))),
    ])
    .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(2, "b"), (7, "g")]));
    db.rollback().unwrap();

    assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
    // The key index was rolled back too: id 7 is free, id 1 is taken.
    db.insert("R", tuple! { id: 7i64, name: "g" }).unwrap();
    assert!(db.insert("R", tuple! { id: 1i64, name: "x" }).is_err());
}

#[test]
fn combined_assignment_is_checked_against_assertions() {
    let mut db = keyed_db();
    db.add_assertion(DatabaseAssertion::new(
        "at_most_three",
        "R holds at most three tuples",
        |db: &mut Database<InMemoryEngine>| {
            db.query("R").map(|r| r.cardinality() <= 3).unwrap_or(false)
        },
    ))
    .unwrap();

    let result = db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(3, "c")]))),
        ("R", Assignment::Insert(r(&[(4, "d")]))),
    ]);
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(contents(&db, "R"), owned(&[(1, "a"), (2, "b")]));
    db.multiple_assign(vec![
        ("R", Assignment::Insert(r(&[(3, "c"), (4, "d")]))),
        ("R", Assignment::Delete(r(&[(1, "a")]))),
    ])
    .unwrap();
    assert_eq!(contents(&db, "R"), owned(&[(2, "b"), (3, "c"), (4, "d")]));
}

#[test]
fn a_relation_converts_into_a_replacement() {
    let assignment: Assignment = r(&[(1, "a")]).into();
    assert!(matches!(assignment, Assignment::Replace(rel) if rel.cardinality() == 1));
}

#[test]
fn empty_multiple_assignment_is_a_no_op() {
    let mut db = cyclic_db();
    db.multiple_assign(Vec::<(&str, Relation)>::new()).unwrap();
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn rollback_undoes_multiple_assignment() {
    let mut db = cyclic_db();
    db.begin().unwrap();
    db.multiple_assign(pair()).unwrap();
    assert_eq!(cardinalities(&db), (1, 1));
    db.rollback().unwrap();

    assert_eq!(cardinalities(&db), (0, 0));
    // The key index was rolled back too: a_id 1 is free again.
    db.multiple_assign(pair()).unwrap();
}

#[test]
fn committed_multiple_assignment_inside_transaction_persists() {
    let mut db = cyclic_db();
    db.begin().unwrap();
    db.multiple_assign(pair()).unwrap();
    db.commit().unwrap();

    assert_eq!(cardinalities(&db), (1, 1));
    assert!(db.verify_integrity().is_ok());
}
