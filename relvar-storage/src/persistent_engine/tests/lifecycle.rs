//! Regression tests: only a live transaction can be committed or rolled
//! back (TTM RM Prescription 17 — a transaction ends exactly once).
//!
//! `PersistentSnapshot` is `Clone`, and the pool's `can_finish` answers
//! `Ok` for unknown ids, so committing a stale snapshot after a rollback
//! used to log a Commit record and make the aborted transaction's
//! versions visible.

use super::common::*;
use crate::persistent_engine::*;
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use tempfile::TempDir;

fn commit_records(engine: &mut PersistentEngine, txn_id: TransactionId) -> usize {
    engine
        .wal
        .scan()
        .unwrap()
        .iter()
        .filter(|(_, r)| matches!(r, WalRecord::Commit { txn_id: t } if *t == txn_id))
        .count()
}

fn abort_records(engine: &mut PersistentEngine, txn_id: TransactionId) -> usize {
    engine
        .wal
        .scan()
        .unwrap()
        .iter()
        .filter(|(_, r)| matches!(r, WalRecord::Abort { txn_id: t } if *t == txn_id))
        .count()
}

#[test]
fn test_commit_after_rollback_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    let stale = snapshot.clone();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.rollback_transaction(snapshot).unwrap();

    assert!(
        engine.commit_transaction(stale).is_err(),
        "a rolled-back transaction must not commit"
    );
    assert!(!engine.committed_txns.contains(&txn_id));
    assert_eq!(commit_records(&mut engine, txn_id), 0, "nothing logged");
    assert_eq!(engine.load_relation("TEST").unwrap().cardinality(), 0);

    drop(engine);
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();
    assert_eq!(engine.load_relation("TEST").unwrap().cardinality(), 0);
}

#[test]
fn test_double_commit_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    let stale = snapshot.clone();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.commit_transaction(snapshot).unwrap();

    assert!(engine.commit_transaction(stale).is_err());
    assert_eq!(commit_records(&mut engine, txn_id), 1);
    assert_eq!(engine.load_relation("TEST").unwrap().cardinality(), 1);
}

#[test]
fn test_rollback_after_commit_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    let stale = snapshot.clone();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.commit_transaction(snapshot).unwrap();

    assert!(engine.rollback_transaction(stale).is_err());
    assert_eq!(abort_records(&mut engine, txn_id), 0, "nothing logged");
    assert_eq!(engine.load_relation("TEST").unwrap().cardinality(), 1);
}

#[test]
fn test_double_rollback_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    let stale = snapshot.clone();
    engine.rollback_transaction(snapshot).unwrap();

    assert!(engine.rollback_transaction(stale).is_err());
    assert_eq!(abort_records(&mut engine, txn_id), 1);
}

#[test]
fn test_commit_after_uncertain_commit_is_rejected() {
    // The commit-point flush fails: the engine ends the transaction as
    // committed ("do not blindly retry"). A retry must be refused instead
    // of appending a second Commit record.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let stale = snapshot.clone();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.wal.inject_flush_failure_in(1);
    assert!(engine.commit_transaction(snapshot).is_err());

    assert!(engine.commit_transaction(stale).is_err());
}
