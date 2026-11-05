//! The implicit key under MVCC (TTM RM Proscription 3).
//!
//! Every relation's whole heading is a candidate key, so each logical
//! tuple has a single version chain: inserting a tuple the transaction
//! already sees is a no-op, and two concurrent transactions creating the
//! same tuple is a write conflict (first-updater-wins), like two
//! concurrent deletes. These are the regression tests for the two
//! counterexamples the Stateright model check (`tests/ttm_model_check.rs`)
//! found, plus the consistency of the in-memory implicit-key index that
//! makes the check O(1).

use super::common::*;
use crate::persistent_engine::*;
use relvar_core::storage_engine::{IsolationLevel, StorageEngine, StorageError};
use relvar_core::tuple;
use relvar_core::values::{Relation, Tuple};
use std::path::Path;
use tempfile::TempDir;

const REL: &str = "TEST";

fn alice() -> Tuple {
    tuple! { id: 1i64, name: "Alice" }
}

fn bob() -> Tuple {
    tuple! { id: 2i64, name: "Bob" }
}

/// An engine with relation `TEST` holding the committed tuple Alice.
fn engine_with_alice(dir: &Path) -> PersistentEngine {
    let mut engine = PersistentEngine::open(dir).unwrap();
    engine.create_relation(REL, test_rel_type()).unwrap();
    engine.insert_tuple(REL, alice()).unwrap();
    engine
}

/// Begins a repeatable-read transaction and suspends it.
fn begin_suspended(engine: &mut PersistentEngine) -> PersistentSnapshot {
    let snapshot = engine
        .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
        .unwrap();
    engine.suspend_transaction().unwrap();
    snapshot
}

/// Runs `f` with `txn` as the ambient transaction, then suspends it.
fn within<T>(
    engine: &mut PersistentEngine,
    txn: &PersistentSnapshot,
    f: impl FnOnce(&mut PersistentEngine) -> T,
) -> T {
    engine.resume_transaction(txn).unwrap();
    let result = f(engine);
    engine.suspend_transaction();
    result
}

/// `txn`'s view of the relation as a relation value.
fn view(engine: &mut PersistentEngine, txn: &PersistentSnapshot) -> Relation {
    within(engine, txn, |e| e.load_relation(REL).unwrap())
}

/// Deletes `tuple` from the relation as seen by the ambient transaction.
fn delete(engine: &mut PersistentEngine, tuple: &Tuple) -> Result<(), StorageError> {
    let current = engine.load_relation(REL).unwrap();
    let kept = current.restrict(|t| t != tuple);
    engine.store_relation(REL, &kept)
}

/// Number of physical version slots of the relation, read from the heap
/// file (after flushing it).
fn versions_on_heap(engine: &mut PersistentEngine, dir: &Path) -> usize {
    engine
        .storage_manager
        .write()
        .unwrap()
        .flush_heap_files()
        .unwrap();
    heap_version_slots(dir, REL).len()
}

fn assert_index_consistent(engine: &PersistentEngine) {
    engine
        .storage_manager
        .write()
        .unwrap()
        .check_key_indexes()
        .unwrap();
}

fn is_write_conflict(result: &Result<(), StorageError>) -> bool {
    matches!(result, Err(StorageError::SerializationFailure(_)))
}

#[test]
fn test_insert_of_visible_tuple_is_a_noop() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 1);

    let txn = engine.begin_transaction().unwrap();
    let lsn_before = engine.wal.current_lsn();
    engine.insert_tuple(REL, alice()).unwrap();

    // Nothing WAL-logged, no second physical version.
    assert_eq!(engine.wal.current_lsn(), lsn_before);
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 1);
    engine.commit_transaction(txn).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 1);
}

#[test]
fn test_insert_of_own_tuple_twice_creates_one_version() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let txn = engine.begin_transaction().unwrap();
    engine.insert_tuple(REL, bob()).unwrap();
    engine.insert_tuple(REL, bob()).unwrap();
    engine.commit_transaction(txn).unwrap();

    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
    assert_index_consistent(&engine);
}

#[test]
fn test_autocommit_duplicate_insert_creates_no_version() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    engine.insert_tuple(REL, alice()).unwrap();

    assert_eq!(versions_on_heap(&mut engine, dir.path()), 1);
}

#[test]
fn test_reinsert_after_own_delete_creates_new_version() {
    // Deleting a snapshot tuple then re-inserting it is not a duplicate:
    // the old version is invisible to the transaction (its own delete).
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let txn = engine.begin_transaction().unwrap();
    delete(&mut engine, &alice()).unwrap();
    engine.insert_tuple(REL, alice()).unwrap();
    engine.commit_transaction(txn).unwrap();

    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 1);
}

#[test]
fn test_concurrent_insert_of_same_tuple_conflicts_while_creator_live() {
    // [Begin(0), Insert(0, 2), Begin(1), Insert(1, 2)]: the second insert
    // is a write conflict.
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    let t1 = begin_suspended(&mut engine);
    let lsn_before = engine.wal.current_lsn();
    let result = within(&mut engine, &t1, |e| e.insert_tuple(REL, bob()));

    assert!(is_write_conflict(&result), "got {result:?}");
    // Nothing changed: no WAL record, no version, T1's view intact.
    assert_eq!(engine.wal.current_lsn(), lsn_before);
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
    assert_eq!(view(&mut engine, &t1).cardinality(), 1);

    // Both transactions can still finish; one Bob is committed.
    engine.commit_transaction(t0).unwrap();
    engine.commit_transaction(t1).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 2);
}

#[test]
fn test_concurrent_insert_conflicts_with_creator_committed_after_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t1 = begin_suspended(&mut engine);
    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    engine.commit_transaction(t0).unwrap();

    let result = within(&mut engine, &t1, |e| e.insert_tuple(REL, bob()));
    assert!(is_write_conflict(&result), "got {result:?}");
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
    engine.rollback_transaction(t1).unwrap();
}

#[test]
fn test_insert_conflicts_with_creator_that_began_earlier_and_committed_later() {
    // T0 begins first, T1's snapshot sees T0 as active; T0 commits Bob.
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t0 = begin_suspended(&mut engine);
    let t1 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    engine.commit_transaction(t0).unwrap();

    let result = within(&mut engine, &t1, |e| e.insert_tuple(REL, bob()));
    assert!(is_write_conflict(&result), "got {result:?}");
    engine.rollback_transaction(t1).unwrap();
}

#[test]
fn test_self_deleted_concurrent_version_does_not_conflict() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| {
        e.insert_tuple(REL, bob()).unwrap();
        delete(e, &bob()).unwrap();
    });
    let t1 = begin_suspended(&mut engine);
    within(&mut engine, &t1, |e| e.insert_tuple(REL, bob())).unwrap();

    engine.commit_transaction(t0).unwrap();
    engine.commit_transaction(t1).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 2);
}

#[test]
fn test_aborted_concurrent_version_does_not_conflict() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t0 = begin_suspended(&mut engine);
    let t1 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    engine.rollback_transaction(t0).unwrap();

    within(&mut engine, &t1, |e| e.insert_tuple(REL, bob())).unwrap();
    engine.commit_transaction(t1).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 2);
}

#[test]
fn test_deleted_tuple_committed_before_snapshot_can_be_reinserted() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());
    let txn = engine.begin_transaction().unwrap();
    delete(&mut engine, &alice()).unwrap();
    engine.commit_transaction(txn).unwrap();

    engine.insert_tuple(REL, alice()).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 1);
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
}

#[test]
fn test_committed_delete_is_not_lost_to_a_duplicate_insert() {
    // The model checker's depth-6 counterexample: R = {1} committed;
    // T0 begins and inserts 1; T1 begins; T0 commits; T1 deletes 1 and
    // commits. T1's committed delete must hold.
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, alice())).unwrap();
    let t1 = begin_suspended(&mut engine);
    engine.commit_transaction(t0).unwrap();
    within(&mut engine, &t1, |e| delete(e, &alice())).unwrap();
    engine.commit_transaction(t1).unwrap();

    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 0);
}

#[test]
fn test_store_relation_insert_conflicts_with_concurrent_insert() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    let t1 = begin_suspended(&mut engine);
    let lsn_before = engine.wal.current_lsn();
    let carol = tuple! { id: 3i64, name: "Carol" };
    let result = within(&mut engine, &t1, |e| {
        // Delete Alice, add Bob and Carol: Bob conflicts, so nothing of
        // the statement may land.
        let mut target = Relation::new(test_rel_type());
        target.insert(bob()).unwrap();
        target.insert(carol.clone()).unwrap();
        e.store_relation(REL, &target)
    });

    assert!(is_write_conflict(&result), "got {result:?}");
    assert_eq!(engine.wal.current_lsn(), lsn_before);
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
    let seen = view(&mut engine, &t1);
    assert_eq!(seen.cardinality(), 1);
    assert!(seen.contains(&alice()));
    engine.rollback_transaction(t1).unwrap();
    engine.commit_transaction(t0).unwrap();
}

#[test]
fn test_store_relation_reinserting_visible_tuples_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());
    let current = engine.load_relation(REL).unwrap();
    engine.store_relation(REL, &current).unwrap();
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 1);
}

#[test]
fn test_read_committed_conflicts_only_with_live_creators() {
    // Read Committed takes a fresh snapshot per statement: a committed
    // creation is visible (no-op), only a live one conflicts.
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());

    let t1 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    engine.suspend_transaction();
    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    let live = within(&mut engine, &t1, |e| e.insert_tuple(REL, bob()));
    assert!(is_write_conflict(&live), "got {live:?}");

    engine.commit_transaction(t0).unwrap();
    within(&mut engine, &t1, |e| e.insert_tuple(REL, bob())).unwrap();
    engine.commit_transaction(t1).unwrap();
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
}

#[test]
fn test_insert_into_missing_relation_logs_nothing() {
    let dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(dir.path()).unwrap();
    let txn = engine.begin_transaction().unwrap();
    let lsn_before = engine.wal.current_lsn();
    let result = engine.insert_tuple("MISSING", alice());
    assert!(matches!(result, Err(StorageError::RelationNotFound(_))));
    assert_eq!(engine.wal.current_lsn(), lsn_before);
    engine.rollback_transaction(txn).unwrap();
}

// --- implicit-key index consistency --------------------------------------

#[test]
fn test_key_index_stays_consistent_across_gc() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());
    for i in 2..20i64 {
        engine
            .insert_tuple(REL, tuple! { id: i, name: "x" })
            .unwrap();
    }
    // An aborted insert and a committed delete leave dead versions.
    let txn = engine.begin_transaction().unwrap();
    engine.insert_tuple(REL, bob()).unwrap();
    engine.rollback_transaction(txn).unwrap();
    let txn = engine.begin_transaction().unwrap();
    delete(&mut engine, &alice()).unwrap();
    engine.commit_transaction(txn).unwrap();
    assert_index_consistent(&engine);

    let before = versions_on_heap(&mut engine, dir.path());
    engine.checkpoint().unwrap();
    assert!(versions_on_heap(&mut engine, dir.path()) < before);
    assert_index_consistent(&engine);

    // GC freed slots get reused by later inserts; the index follows.
    engine.insert_tuple(REL, alice()).unwrap();
    engine.insert_tuple(REL, bob()).unwrap();
    assert_index_consistent(&engine);
    engine.insert_tuple(REL, alice()).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 20);
}

#[test]
fn test_key_index_is_rebuilt_after_crash_recovery() {
    let dir = TempDir::new().unwrap();
    {
        let mut engine = engine_with_alice(dir.path());
        let _crashed = engine.begin_transaction().unwrap();
        engine.insert_tuple(REL, bob()).unwrap();
        engine
            .storage_manager
            .write()
            .unwrap()
            .flush_heap_files()
            .unwrap();
        // Dropped with the transaction open: a crash.
    }
    let mut engine = PersistentEngine::open(dir.path()).unwrap();
    // Recovery undo removed the crashed version; the rebuilt index does
    // not see it, so Bob inserts cleanly (no conflict, one version).
    engine.insert_tuple(REL, bob()).unwrap();
    engine.insert_tuple(REL, alice()).unwrap();
    assert_index_consistent(&engine);
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 2);
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 2);
}

#[test]
fn test_key_index_survives_drop_and_recreate_of_relation() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());
    engine.drop_relation(REL).unwrap();
    engine.create_relation(REL, test_rel_type()).unwrap();
    // The new relation is empty: Alice is a fresh tuple, not a duplicate.
    engine.insert_tuple(REL, alice()).unwrap();
    assert_eq!(engine.load_relation(REL).unwrap().cardinality(), 1);
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 1);
    assert_index_consistent(&engine);
}

#[test]
fn test_key_index_overflow_falls_back_to_scans() {
    let dir = TempDir::new().unwrap();
    let mut engine = engine_with_alice(dir.path());
    engine
        .storage_manager
        .write()
        .unwrap()
        .heap_file_for_test(REL)
        .unwrap()
        .set_key_index_capacity(3);
    for i in 2..6i64 {
        engine
            .insert_tuple(REL, tuple! { id: i, name: "x" })
            .unwrap();
    }
    assert!(
        engine
            .storage_manager
            .write()
            .unwrap()
            .heap_file_for_test(REL)
            .unwrap()
            .key_index_overflowed()
    );

    // Duplicates are still no-ops and conflicts still detected.
    engine.insert_tuple(REL, alice()).unwrap();
    assert_eq!(versions_on_heap(&mut engine, dir.path()), 5);
    let t0 = begin_suspended(&mut engine);
    within(&mut engine, &t0, |e| e.insert_tuple(REL, bob())).unwrap();
    let t1 = begin_suspended(&mut engine);
    let result = within(&mut engine, &t1, |e| e.insert_tuple(REL, bob()));
    assert!(is_write_conflict(&result), "got {result:?}");
    engine.rollback_transaction(t0).unwrap();
    engine.rollback_transaction(t1).unwrap();

    // GC reclaims the aborted version and retries the index, which fits
    // after deletes shrink the relation.
    let txn = engine.begin_transaction().unwrap();
    engine
        .store_relation(REL, &Relation::new(test_rel_type()))
        .unwrap();
    engine.commit_transaction(txn).unwrap();
    engine.checkpoint().unwrap();
    engine.insert_tuple(REL, alice()).unwrap();
    let mut manager = engine.storage_manager.write().unwrap();
    let heap = manager.heap_file_for_test(REL).unwrap();
    assert!(heap.key_index_is_built());
    drop(manager);
    assert_index_consistent(&engine);
}
