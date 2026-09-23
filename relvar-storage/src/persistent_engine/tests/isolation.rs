//! Behavioral tests for per-transaction isolation levels.
//!
//! The engine is single-threaded, so "concurrent" transactions interleave in
//! time: T1 begins, is suspended, T2 runs to commit, then T1 resumes. The
//! explicit `*_for_txn` engine API addresses each transaction by id, so the
//! two transactions' snapshots never interfere with each other.
//!
//! Covered:
//! - ReadCommitted: a transaction re-reading a relation observes a
//!   transaction that committed in between (non-repeatable read).
//! - RepeatableRead: the same interleaving is invisible; re-reads are stable.
//! - Serializable: commit fails with `StorageError::SerializationFailure`
//!   when a concurrent transaction committed overlapping changes
//!   (first-committer-wins), for both write/write and read/write overlap.

use super::common::*;
use crate::persistent_engine::*;
use crate::wal::TransactionId;
use relvar_core::storage_engine::{IsolationLevel, StorageEngine, StorageError};
use relvar_core::tuple;
use relvar_core::values::ScalarValue;
use tempfile::TempDir;

/// Opens an engine with a PEOPLE(id Int, name String) relation holding one
/// committed tuple: (1, "Alice").
fn setup_engine() -> (TempDir, PersistentEngine) {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("PEOPLE", test_rel_type()).unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    (temp_dir, engine)
}

/// Names visible in PEOPLE for the given transaction.
fn visible_names(engine: &PersistentEngine, txn_id: TransactionId) -> Vec<String> {
    let relation = engine.load_relation_for_txn("PEOPLE", txn_id).unwrap();
    let mut names: Vec<String> = relation
        .tuples()
        .filter_map(|t| match t.get("name") {
            Some(ScalarValue::String(name)) => Some(name.clone()),
            _ => None,
        })
        .collect();
    names.sort();
    names
}

/// Begins a transaction and immediately suspends it, returning its snapshot.
/// The engine is left with no ambient transaction.
fn begin_suspended(engine: &mut PersistentEngine, level: IsolationLevel) -> PersistentSnapshot {
    let snapshot = engine.begin_transaction_with_isolation(level).unwrap();
    let suspended = engine.suspend_transaction().unwrap();
    assert_eq!(suspended.txn_id, snapshot.txn_id);
    snapshot
}

#[test]
fn read_committed_permits_non_repeatable_reads() {
    let (_dir, mut engine) = setup_engine();

    // T1 (ReadCommitted) reads PEOPLE: sees Alice.
    let t1 = begin_suspended(&mut engine, IsolationLevel::ReadCommitted);
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice"]);

    // A concurrent transaction inserts Bob and commits.
    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t2.txn_id)
        .unwrap();
    engine.commit_transaction(t2).unwrap();

    // T1 re-reads: ReadCommitted observes the latest committed state,
    // so Bob is now visible mid-transaction (non-repeatable read).
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice", "Bob"]);

    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn repeatable_read_prevents_non_repeatable_reads() {
    let (_dir, mut engine) = setup_engine();

    // T1 (RepeatableRead) reads PEOPLE: sees Alice.
    let t1 = begin_suspended(&mut engine, IsolationLevel::RepeatableRead);
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice"]);

    // A concurrent transaction inserts Bob and commits.
    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t2.txn_id)
        .unwrap();
    engine.commit_transaction(t2).unwrap();

    // T1 re-reads: its begin snapshot is fixed, so Bob stays invisible.
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice"]);

    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();

    // After T1 commits, a new transaction sees both tuples.
    let t3 = engine.begin_transaction().unwrap();
    assert_eq!(visible_names(&engine, t3.txn_id), vec!["Alice", "Bob"]);
    engine.commit_transaction(t3).unwrap();
}

#[test]
fn read_committed_hides_uncommitted_changes() {
    let (_dir, mut engine) = setup_engine();

    let t1 = begin_suspended(&mut engine, IsolationLevel::ReadCommitted);
    let t2 = begin_suspended(&mut engine, IsolationLevel::ReadCommitted);
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t2.txn_id)
        .unwrap();

    // Bob is uncommitted: invisible even to ReadCommitted...
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice"]);
    // ... but T2 sees its own write.
    assert_eq!(visible_names(&engine, t2.txn_id), vec!["Alice", "Bob"]);

    engine.resume_transaction(&t2).unwrap();
    engine.commit_transaction(t2).unwrap();

    // Now committed: visible to T1's next read.
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice", "Bob"]);

    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn serializable_aborts_on_write_write_conflict() {
    let (_dir, mut engine) = setup_engine();

    // T1 (Serializable) writes PEOPLE.
    let t1 = begin_suspended(&mut engine, IsolationLevel::Serializable);
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t1.txn_id)
        .unwrap();

    // A concurrent transaction also writes PEOPLE and commits first.
    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 3i64, name: "Carol" }, t2.txn_id)
        .unwrap();
    engine.commit_transaction(t2).unwrap();

    // T1 is the loser: first-committer-wins.
    engine.resume_transaction(&t1).unwrap();
    let err = engine.commit_transaction(t1).unwrap_err();
    assert!(
        matches!(err, StorageError::SerializationFailure(_)),
        "expected SerializationFailure, got {err:?}"
    );

    // T1's write was rolled back: Bob is gone, Carol (committed) remains.
    let t3 = engine.begin_transaction().unwrap();
    assert_eq!(visible_names(&engine, t3.txn_id), vec!["Alice", "Carol"]);
    engine.commit_transaction(t3).unwrap();
}

#[test]
fn serializable_aborts_on_read_write_phantom() {
    let (_dir, mut engine) = setup_engine();

    // T1 (Serializable) reads PEOPLE.
    let t1 = begin_suspended(&mut engine, IsolationLevel::Serializable);
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice"]);

    // A concurrent transaction inserts Bob and commits.
    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t2.txn_id)
        .unwrap();
    engine.commit_transaction(t2).unwrap();

    // Committing T1 would admit a phantom: abort instead.
    engine.resume_transaction(&t1).unwrap();
    let err = engine.commit_transaction(t1).unwrap_err();
    assert!(
        matches!(err, StorageError::SerializationFailure(_)),
        "expected SerializationFailure, got {err:?}"
    );
}

#[test]
fn serializable_commits_without_overlap() {
    let (_dir, mut engine) = setup_engine();

    // T1 (Serializable) writes PEOPLE; the concurrent transaction only reads.
    let t1 = begin_suspended(&mut engine, IsolationLevel::Serializable);
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t1.txn_id)
        .unwrap();

    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    assert_eq!(visible_names(&engine, t2.txn_id), vec!["Alice"]);
    engine.commit_transaction(t2).unwrap();

    // No overlapping writes: T1 commits cleanly.
    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();

    let t3 = engine.begin_transaction().unwrap();
    assert_eq!(visible_names(&engine, t3.txn_id), vec!["Alice", "Bob"]);
    engine.commit_transaction(t3).unwrap();
}

#[test]
fn serializable_sees_own_writes() {
    let (_dir, mut engine) = setup_engine();

    let t1 = begin_suspended(&mut engine, IsolationLevel::Serializable);
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t1.txn_id)
        .unwrap();
    // Fixed begin snapshot plus own-write visibility: Bob is visible to T1.
    assert_eq!(visible_names(&engine, t1.txn_id), vec!["Alice", "Bob"]);

    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn begin_displaces_ambient_transaction() {
    let (_dir, mut engine) = setup_engine();

    // Beginning a second transaction displaces (but does not end) the first:
    // both stay live and committable through their own snapshot handles.
    let t1 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    assert_ne!(t1.txn_id, t2.txn_id);

    engine.commit_transaction(t2).unwrap();
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn suspend_resume_round_trip() {
    let (_dir, mut engine) = setup_engine();

    assert!(engine.suspend_transaction().is_none());

    let t1 = engine
        .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
        .unwrap();
    let suspended = engine.suspend_transaction().unwrap();
    assert_eq!(suspended.txn_id, t1.txn_id);

    // A dead snapshot cannot be resumed.
    let t2 = engine.begin_transaction().unwrap();
    let t2_id = t2.txn_id;
    engine.commit_transaction(t2).unwrap();
    let stale = PersistentSnapshot { txn_id: t2_id };
    let err = engine.resume_transaction(&stale).unwrap_err();
    assert!(matches!(err, StorageError::Other(_)));

    engine.resume_transaction(&suspended).unwrap();
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn failed_commit_strands_no_transaction() {
    let (_dir, mut engine) = setup_engine();

    // T1 (Serializable) writes PEOPLE; T2 concurrently writes and commits.
    let t1 = begin_suspended(&mut engine, IsolationLevel::Serializable);
    let t1_id = t1.txn_id;
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 2i64, name: "Bob" }, t1_id)
        .unwrap();
    let t2 = engine
        .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
        .unwrap();
    engine
        .insert_tuple_in_txn("PEOPLE", tuple! { id: 3i64, name: "Carol" }, t2.txn_id)
        .unwrap();
    engine.commit_transaction(t2).unwrap();

    // T1's commit fails the serializable validation...
    engine.resume_transaction(&t1).unwrap();
    let err = engine.commit_transaction(t1).unwrap_err();
    assert!(matches!(err, StorageError::SerializationFailure(_)));

    // ...and the engine is left idle: no ambient transaction, and the
    // aborted transaction is gone from the active table (contract:
    // `StorageEngine::commit_transaction` never strands a transaction
    // on error).
    assert_eq!(engine.current_txn, None);
    assert!(engine.active_txns.get_snapshot(t1_id).is_none());

    // A fresh transaction begins cleanly on the idle engine.
    let t3 = engine.begin_transaction().unwrap();
    assert_eq!(visible_names(&engine, t3.txn_id), vec!["Alice", "Carol"]);
    engine.commit_transaction(t3).unwrap();
}
