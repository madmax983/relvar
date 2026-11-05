//! Multiple assignment (TTM RM Prescription 21) on the persistent engine:
//! several relvar assignments as one statement, checked only on the final
//! state and applied all-or-nothing, inside and outside a transaction.

#![cfg(feature = "storage")]

use relvar::constraints::{
    ConstraintManagerError, DatabaseAssertion, ForeignKey, ForeignKeyConstraints, KeyConstraints,
    PrimaryKey,
};
use relvar::{
    Database, DatabaseError, PersistentEngine, Relation, RelationType, ScalarType, Tuple,
    TupleType, tuple,
};
use std::path::Path;

type Db = Database<PersistentEngine>;

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

fn rel(ty: RelationType, tuples: Vec<Tuple>) -> Relation {
    Relation::from_tuples(ty, tuples).unwrap()
}

fn pk(attribute: &str) -> KeyConstraints {
    KeyConstraints::new().with_primary_key(PrimaryKey::new(vec![attribute.to_string()]).unwrap())
}

fn fk(from: &str, to_relvar: &str, to: &str) -> ForeignKeyConstraints {
    ForeignKeyConstraints::new().with_foreign_key(
        ForeignKey::new(
            vec![from.to_string()],
            to_relvar.to_string(),
            vec![to.to_string()],
        )
        .unwrap(),
    )
}

/// A and B keyed on their ids and referencing each other.
fn cyclic_db(dir: &Path) -> Db {
    let mut db = Database::new(PersistentEngine::open(dir).unwrap());
    db.create_relvar("A", a_type()).unwrap();
    db.create_relvar("B", b_type()).unwrap();
    db.set_key_constraints("A", pk("a_id")).unwrap();
    db.set_key_constraints("B", pk("b_id")).unwrap();
    db.set_foreign_key_constraints("A", fk("b_ref", "B", "b_id"))
        .unwrap();
    db.set_foreign_key_constraints("B", fk("a_ref", "A", "a_id"))
        .unwrap();
    db
}

fn pair(a_ref: i64) -> Vec<(&'static str, Relation)> {
    vec![
        ("A", rel(a_type(), vec![tuple! { a_id: 1i64, b_ref: 1i64 }])),
        (
            "B",
            rel(b_type(), vec![tuple! { b_id: 1i64, a_ref: a_ref }]),
        ),
    ]
}

fn cardinalities(db: &Db) -> (usize, usize) {
    (
        db.query("A").unwrap().cardinality(),
        db.query("B").unwrap().cardinality(),
    )
}

#[test]
fn cyclic_relvars_are_populated_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = cyclic_db(dir.path());
        assert!(db.insert("A", tuple! { a_id: 1i64, b_ref: 1i64 }).is_err());
        db.multiple_assign(pair(1)).unwrap();
        assert_eq!(cardinalities(&db), (1, 1));
    }
    let mut db = Database::new(PersistentEngine::open(dir.path()).unwrap());
    assert_eq!(cardinalities(&db), (1, 1));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn failing_multiple_assignment_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = cyclic_db(dir.path());
    let result = db.multiple_assign(pair(2));

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn assertion_violation_undoes_both_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = cyclic_db(dir.path());
    db.add_assertion(DatabaseAssertion::new(
        "b_empty",
        "B holds no tuples",
        |db: &mut Db| db.query("B").map(|b| b.is_empty()).unwrap_or(false),
    ))
    .unwrap();

    assert!(matches!(
        db.multiple_assign(pair(1)),
        Err(DatabaseError::AssertionViolation(_))
    ));
    assert_eq!(cardinalities(&db), (0, 0));

    // The same holds inside a transaction, which stays usable.
    db.begin().unwrap();
    assert!(db.multiple_assign(pair(1)).is_err());
    assert_eq!(cardinalities(&db), (0, 0));
    db.commit().unwrap();
    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn rollback_undoes_multiple_assignment() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = cyclic_db(dir.path());
    db.begin().unwrap();
    db.multiple_assign(pair(1)).unwrap();
    assert_eq!(cardinalities(&db), (1, 1));
    db.rollback().unwrap();

    assert_eq!(cardinalities(&db), (0, 0));
}

#[test]
fn committed_multiple_assignment_inside_transaction_persists() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = cyclic_db(dir.path());
        db.begin().unwrap();
        db.multiple_assign(pair(1)).unwrap();
        db.commit().unwrap();
    }
    let db = Database::new(PersistentEngine::open(dir.path()).unwrap());
    assert_eq!(cardinalities(&db), (1, 1));
}
