//! Candidate key regression tests (TTM RM Prescription 15).
//!
//! The empty key is legal (a relvar with key {} holds at most one tuple),
//! key attributes must exist and be distinct, and candidate keys are
//! irreducible.

use super::golden_rule::id_name_type;
use crate::constraints::{CandidateKey, ConstraintManagerError, KeyConstraints, PrimaryKey};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use alloc::string::ToString;
use alloc::vec;

pub(super) fn keys(primary: Option<&[&str]>, candidates: &[&[&str]]) -> KeyConstraints {
    let to_vec = |attrs: &[&str]| attrs.iter().map(|a| a.to_string()).collect();
    let mut constraints = KeyConstraints::new();
    if let Some(attrs) = primary {
        constraints = constraints.with_primary_key(PrimaryKey::new(to_vec(attrs)).unwrap());
    }
    for attrs in candidates {
        constraints = constraints.with_candidate_key(CandidateKey::new(to_vec(attrs)).unwrap());
    }
    constraints
}

#[test]
fn empty_key_is_a_legal_candidate_key() {
    assert!(CandidateKey::new(vec![]).is_ok());
    assert!(PrimaryKey::new(vec![]).is_ok());
}

#[test]
fn empty_key_admits_at_most_one_tuple() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("SINGLETON", id_name_type()).unwrap();
    db.set_key_constraints("SINGLETON", keys(None, &[&[]]))
        .unwrap();

    db.insert("SINGLETON", tuple! { id: 1i64, name: "only" })
        .unwrap();
    let second = db.insert("SINGLETON", tuple! { id: 2i64, name: "second" });
    assert!(matches!(
        second,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CandidateKeyViolation
        ))
    ));
    assert_eq!(db.query("SINGLETON").unwrap().cardinality(), 1);
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn empty_key_rejects_multi_tuple_bulk_insert_but_allows_update() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("SINGLETON", id_name_type()).unwrap();
    db.set_key_constraints("SINGLETON", keys(Some(&[]), &[]))
        .unwrap();

    let batch = db.bulk_insert(
        "SINGLETON",
        vec![
            tuple! { id: 1i64, name: "a" },
            tuple! { id: 2i64, name: "b" },
        ],
    );
    assert!(batch.is_err());
    assert_eq!(db.query("SINGLETON").unwrap().cardinality(), 0);

    db.insert("SINGLETON", tuple! { id: 1i64, name: "a" })
        .unwrap();
    // Replacing the single tuple keeps the relvar a singleton.
    db.update("SINGLETON", |_| true, |_| tuple! { id: 2i64, name: "b" })
        .unwrap();
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn empty_key_is_rejected_when_existing_data_has_two_tuples() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    db.insert("R", tuple! { id: 1i64, name: "a" }).unwrap();
    db.insert("R", tuple! { id: 2i64, name: "b" }).unwrap();
    assert!(db.set_key_constraints("R", keys(None, &[&[]])).is_err());
    assert!(db.get_key_constraints("R").is_none());
}

#[test]
fn key_naming_unknown_attribute_is_rejected() {
    // On an empty relvar no tuple forces attribute extraction, so the
    // heading must be checked explicitly.
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    let result = db.set_key_constraints("R", keys(None, &[&["id", "ghost"]]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::AttributeNotFound(attr, rel)
        )) if attr == "ghost" && rel == "R"
    ));
    assert!(db.get_key_constraints("R").is_none());
}

#[test]
fn key_with_duplicate_attribute_name_is_rejected() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    let result = db.set_key_constraints("R", keys(Some(&["id", "id"]), &[]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::DuplicateKeyAttribute { .. }
        ))
    ));
}

#[test]
fn reducible_key_is_rejected() {
    // {id, name} is a proper superset of the key {id}: a superkey, not a
    // candidate key (candidate keys are irreducible).
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    let result = db.set_key_constraints("R", keys(Some(&["id"]), &[&["name", "id"]]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ReducibleKey { .. }
        ))
    ));

    // Every non-empty key is a proper superset of the empty key.
    let result = db.set_key_constraints("R", keys(None, &[&[], &["id"]]));
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ReducibleKey { .. }
        ))
    ));
    assert!(db.get_key_constraints("R").is_none());
}

#[test]
fn independent_keys_are_accepted() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("R", id_name_type()).unwrap();
    assert!(
        db.set_key_constraints("R", keys(Some(&["id"]), &[&["name"]]))
            .is_ok()
    );
}
