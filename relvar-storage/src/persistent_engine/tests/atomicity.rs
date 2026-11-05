//! Regression tests: `store_relation` (the delete/update path) is atomic,
//! durable and isolated under MVCC (TTM RM Prescription 17).
//!
//! `store_relation` used to rebuild the heap from scratch with every tuple
//! stamped `xmin = txn` and rename it over the old heap, so the committed
//! versions were physically destroyed: a rollback (or a crash before
//! commit) left the relation empty, concurrent snapshots lost their
//! versions, and concurrent transactions' uncommitted inserts were wiped.
//! It now applies a versioned diff (deletes set `xmax`, inserts add
//! versions), WAL-logged, so abort and crash recovery can undo it.

use super::common::*;
use crate::persistent_engine::*;
use relvar_core::database::Database;
use relvar_core::storage_engine::{IsolationLevel, StorageEngine, StorageError};
use relvar_core::tuple;
use relvar_core::values::{Relation, ScalarValue};
use tempfile::TempDir;

/// Opens an engine with PEOPLE holding the committed tuples
/// (1, Alice) and (2, Bob).
fn setup() -> (TempDir, PersistentEngine) {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("PEOPLE", test_rel_type()).unwrap();
    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 2i64, name: "Bob" })
        .unwrap();
    engine.commit_transaction(snapshot).unwrap();
    (temp_dir, engine)
}

/// Sorted names of a relation's tuples.
fn names(relation: &Relation) -> Vec<String> {
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

/// PEOPLE with only (1, Alice): the result of deleting Bob.
fn only_alice() -> Relation {
    let mut relation = Relation::new(test_rel_type());
    relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    relation
}

#[test]
fn test_rollback_after_store_relation_delete_restores_committed_tuples() {
    let (_dir, mut engine) = setup();

    let snapshot = engine.begin_transaction().unwrap();
    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );
    engine.rollback_transaction(snapshot).unwrap();

    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob"],
        "rollback must restore the committed tuples a delete removed"
    );
}

#[test]
fn test_database_delete_then_rollback_restores_relation() {
    let (_dir, engine) = setup();
    let mut db = Database::new(engine);

    db.begin().unwrap();
    let deleted = db
        .delete("PEOPLE", |t| t.get_typed::<i64>("id").unwrap() == 2)
        .unwrap();
    assert_eq!(deleted, 1);
    db.rollback().unwrap();

    assert_eq!(names(&db.query("PEOPLE").unwrap()), vec!["Alice", "Bob"]);
}

#[test]
fn test_database_update_then_rollback_restores_relation() {
    let (_dir, engine) = setup();
    let mut db = Database::new(engine);

    db.begin().unwrap();
    db.update(
        "PEOPLE",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_| tuple! { id: 2i64, name: "Robert" },
    )
    .unwrap();
    assert_eq!(names(&db.query("PEOPLE").unwrap()), vec!["Alice", "Robert"]);
    db.rollback().unwrap();

    assert_eq!(names(&db.query("PEOPLE").unwrap()), vec!["Alice", "Bob"]);
}

#[test]
fn test_crash_after_uncommitted_delete_keeps_committed_tuples() {
    let (dir, mut engine) = setup();

    let _snapshot = engine.begin_transaction().unwrap();
    engine
        .store_relation("PEOPLE", &Relation::new(test_rel_type()))
        .unwrap();
    // Crash: the delete never commits.
    drop(engine);

    let engine = PersistentEngine::open(dir.path()).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob"],
        "an uncommitted delete must not survive a crash"
    );
}

#[test]
fn test_committed_delete_survives_reopen() {
    let (dir, mut engine) = setup();

    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    drop(engine);

    let engine = PersistentEngine::open(dir.path()).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );
}

#[test]
fn test_recovery_clears_uncommitted_delete_marks() {
    let (dir, mut engine) = setup();

    let snapshot = engine.begin_transaction().unwrap();
    let crashed = snapshot.txn_id;
    engine
        .store_relation("PEOPLE", &Relation::new(test_rel_type()))
        .unwrap();
    // Force the delete marks to the heap file, then crash before commit.
    engine
        .storage_manager
        .write()
        .unwrap()
        .flush_heap_files()
        .unwrap();
    assert!(
        heap_version_slots(dir.path(), "PEOPLE")
            .iter()
            .any(|slot| slot.xmax == Some(crashed)),
        "the uncommitted delete must be a versioned xmax mark"
    );
    drop(engine);

    let _engine = PersistentEngine::open(dir.path()).unwrap();
    let slots = heap_version_slots(dir.path(), "PEOPLE");
    assert_eq!(slots.len(), 2, "both committed versions are kept");
    assert!(
        slots.iter().all(|slot| slot.xmax.is_none()),
        "recovery must clear the crashed transaction's delete marks: {slots:?}"
    );
}

#[test]
fn test_recovery_removes_uncommitted_inserts_physically() {
    let (dir, mut engine) = setup();

    let _snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 3i64, name: "Carol" })
        .unwrap();
    engine
        .storage_manager
        .write()
        .unwrap()
        .flush_heap_files()
        .unwrap();
    assert_eq!(heap_version_slots(dir.path(), "PEOPLE").len(), 3);
    drop(engine);

    let engine = PersistentEngine::open(dir.path()).unwrap();
    assert_eq!(heap_version_slots(dir.path(), "PEOPLE").len(), 2);
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob"]
    );
}

#[test]
fn test_repeatable_read_snapshot_keeps_seeing_concurrently_deleted_tuples() {
    let (_dir, mut engine) = setup();

    // T1 (RepeatableRead) begins before the delete commits.
    let t1 = engine
        .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
        .unwrap();
    engine.suspend_transaction().unwrap();

    // T2 deletes Bob and commits.
    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );

    // A checkpoint (GC) must not reclaim the version T1 can still see.
    engine.checkpoint().unwrap();

    engine.resume_transaction(&t1).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob"],
        "a snapshot predating the delete must keep seeing the deleted tuple"
    );
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn test_gc_reclaims_deleted_versions_once_no_snapshot_can_see_them() {
    let (dir, mut engine) = setup();

    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    assert_eq!(heap_version_slots(dir.path(), "PEOPLE").len(), 2);

    engine.checkpoint().unwrap();
    let slots = heap_version_slots(dir.path(), "PEOPLE");
    assert_eq!(slots.len(), 1, "the dead version is reclaimed: {slots:?}");
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );
}

#[test]
fn test_store_relation_preserves_concurrent_uncommitted_inserts() {
    let (_dir, mut engine) = setup();

    // T1 inserts Carol but does not commit yet.
    let t1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 3i64, name: "Carol" })
        .unwrap();
    engine.suspend_transaction().unwrap();

    // T2 deletes Bob and commits.
    engine.store_relation("PEOPLE", &only_alice()).unwrap();

    // T1 commits: its insert must have survived T2's store.
    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();

    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Carol"]
    );
}

#[test]
fn test_store_relation_preserves_inserts_committed_after_snapshot() {
    let (_dir, mut engine) = setup();

    // T1 (RepeatableRead) reads PEOPLE.
    let t1 = engine
        .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
        .unwrap();
    let seen = engine.load_relation("PEOPLE").unwrap();
    engine.suspend_transaction().unwrap();

    // T2 inserts Carol and commits.
    engine
        .insert_tuple("PEOPLE", tuple! { id: 3i64, name: "Carol" })
        .unwrap();

    // T1 deletes Bob from what it saw and commits.
    engine.resume_transaction(&t1).unwrap();
    let mut without_bob = Relation::new(test_rel_type());
    for tuple in seen.tuples() {
        if tuple.get_typed::<i64>("id").unwrap() != 2 {
            without_bob.insert(tuple.clone()).unwrap();
        }
    }
    engine.store_relation("PEOPLE", &without_bob).unwrap();
    engine.commit_transaction(t1).unwrap();

    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Carol"],
        "T1's delete must not wipe a tuple committed after its snapshot"
    );
}

#[test]
fn test_concurrent_delete_of_same_tuple_is_a_write_conflict() {
    let (_dir, mut engine) = setup();

    // T1 deletes Bob, uncommitted.
    let t1 = engine.begin_transaction().unwrap();
    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    engine.suspend_transaction().unwrap();

    // T2 also tries to delete Bob: first-updater-wins.
    let t2 = engine.begin_transaction().unwrap();
    let result = engine.store_relation("PEOPLE", &only_alice());
    assert!(
        matches!(result, Err(StorageError::SerializationFailure(_))),
        "expected a write conflict, got {result:?}"
    );
    engine.rollback_transaction(t2).unwrap();

    // T1 is unaffected and commits its delete.
    engine.resume_transaction(&t1).unwrap();
    engine.commit_transaction(t1).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );
}

#[test]
fn test_delete_of_tuple_deleted_by_aborted_txn_succeeds() {
    let (dir, mut engine) = setup();

    let t1 = engine.begin_transaction().unwrap();
    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    engine.rollback_transaction(t1).unwrap();

    // The aborted delete mark does not block a later delete.
    engine.store_relation("PEOPLE", &only_alice()).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );
    drop(engine);
    let engine = PersistentEngine::open(dir.path()).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice"]
    );
}

#[test]
fn test_store_relation_logs_delete_and_insert_records() {
    let (_dir, mut engine) = setup();

    let mut relation = only_alice();
    relation.insert(tuple! { id: 3i64, name: "Carol" }).unwrap();
    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    engine.store_relation("PEOPLE", &relation).unwrap();
    engine.commit_transaction(snapshot).unwrap();

    let records = engine.wal.scan().unwrap();
    let deletes = records
        .iter()
        .filter(|(_, r)| matches!(r, WalRecord::Delete { txn_id: t, .. } if *t == txn_id))
        .count();
    let inserts = records
        .iter()
        .filter(|(_, r)| matches!(r, WalRecord::Insert { txn_id: t, .. } if *t == txn_id))
        .count();
    assert_eq!((deletes, inserts), (1, 1), "only the diff is logged");
}

#[test]
fn test_failed_autocommit_store_relation_leaves_no_live_transaction() {
    let temp_dir = TempDir::new().unwrap();
    // Two version records: deleting two tuples and inserting one more
    // needs three.
    let mut engine =
        PersistentEngine::open_with_pool_capacities(temp_dir.path(), 16, 2, 2).unwrap();
    engine.create_relation("PEOPLE", test_rel_type()).unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    let mut replacement = Relation::new(test_rel_type());
    replacement
        .insert(tuple! { id: 3i64, name: "Carol" })
        .unwrap();
    let result = engine.store_relation("PEOPLE", &replacement);
    assert!(
        matches!(result, Err(StorageError::VersionPoolExhausted { .. })),
        "got {result:?}"
    );
    assert!(engine.current_txn.is_none(), "the autocommit txn must end");
    assert_eq!(engine.txn_pool.active_count(), 0);
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob"]
    );
}

#[test]
fn test_failed_store_relation_in_explicit_txn_is_statement_atomic() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine =
        PersistentEngine::open_with_pool_capacities(temp_dir.path(), 16, 3, 2).unwrap();
    engine.create_relation("PEOPLE", test_rel_type()).unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("PEOPLE", tuple! { id: 3i64, name: "Carol" })
        .unwrap();
    // Needs 2 deletes + 2 inserts = 4 records; only 2 remain.
    let mut replacement = Relation::new(test_rel_type());
    replacement
        .insert(tuple! { id: 4i64, name: "Dan" })
        .unwrap();
    replacement
        .insert(tuple! { id: 5i64, name: "Eve" })
        .unwrap();
    replacement
        .insert(tuple! { id: 3i64, name: "Carol" })
        .unwrap();
    assert!(engine.store_relation("PEOPLE", &replacement).is_err());

    // The failed statement left no partial effects in the transaction.
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob", "Carol"]
    );
    engine.commit_transaction(snapshot).unwrap();
    assert_eq!(
        names(&engine.load_relation("PEOPLE").unwrap()),
        vec!["Alice", "Bob", "Carol"]
    );
}
