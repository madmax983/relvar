use super::common::*;
use crate::persistent_engine::*;
use relvar_core::tuple;
use relvar_storage_core::slotted::{PAGE_FORMAT_VERSION, VersionedSlottedPage};
use std::path::Path;
use tempfile::TempDir;

/// Counts the live version slots on page 0 of `<dir>/TEST.heap` by decoding
/// the page image straight from the file. This lets these tests observe
/// physical garbage collection without touching private engine state: heap
/// pages are `PAGE_SIZE` blocks framed as `[u64 len][page data]`, and page
/// data starts with the v2 header `[format version][u32 slot-dir len]`.
fn count_heap_versions(dir: &Path) -> usize {
    let bytes = std::fs::read(dir.join("TEST.heap")).unwrap();
    let len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
    let data = &bytes[8..8 + len];
    assert_eq!(data[0], PAGE_FORMAT_VERSION);
    let slot_dir_len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
    let page: VersionedSlottedPage = postcard::from_bytes(&data[5..5 + slot_dir_len]).unwrap();
    page.slots.iter().flatten().count()
}

#[test]
fn test_abort_wal_flush_failure_leaves_txn_live_and_retryable() {
    // `abort_txn` must not recycle the pool slot until the Abort record is
    // durable. An injected flush failure therefore leaves the transaction
    // live (retryable) instead of half-aborted.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    engine.wal.inject_next_flush_failure();
    let result = engine.rollback_transaction(snapshot);

    // The abort reports the flush failure honestly...
    assert!(result.is_err(), "abort must surface the WAL flush failure");
    assert!(
        result.unwrap_err().to_string().contains("flush"),
        "error should name the flush"
    );

    // ...but the transaction is still live and retryable: not aborted in
    // the pool, not in the committed set, not half-cleaned.
    assert!(
        engine.txn_pool.get_snapshot(txn_id).is_some(),
        "transaction must stay live after a failed abort flush"
    );
    assert!(!engine.committed_txns.contains(&txn_id));

    // Retrying the abort (with a healthy WAL) completes it.
    engine
        .rollback_transaction(PersistentSnapshot { txn_id })
        .unwrap();
    assert!(engine.txn_pool.get_snapshot(txn_id).is_none());
    assert!(!engine.committed_txns.contains(&txn_id));

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 0);
}

#[test]
fn test_abort_record_is_durable_across_crash() {
    // The Abort record is flushed before the pool slot is recycled, so a
    // crash immediately after the abort replays the abort from the WAL —
    // the record is in the WAL file before the engine is even dropped.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.rollback_transaction(snapshot).unwrap();

    // The WAL buffer is empty: everything (Begin, Insert, Abort) reached
    // the file before the pool forgot the transaction.
    assert!(
        engine.wal.is_buffer_empty(),
        "abort must flush its WAL record before returning"
    );

    // Simulate a crash: drop without checkpointing, reopen, recover.
    drop(engine);
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    // The aborted tuple stays invisible after recovery.
    let snapshot = engine.begin_transaction().unwrap();
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 0);
    engine.commit_transaction(snapshot).unwrap();
}

#[test]
fn test_gc_reclaims_aborted_insert_after_checkpoint() {
    // End-to-end: an aborted insert is physically on the heap page, and a
    // checkpoint's GC pass reclaims it via the creator-death rule.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.rollback_transaction(snapshot).unwrap();

    // The aborted version is physically present (writes are write-through).
    assert_eq!(count_heap_versions(temp_dir.path()), 1);

    engine.checkpoint().unwrap();

    // GC reclaimed it: the slot is gone.
    assert_eq!(
        count_heap_versions(temp_dir.path()),
        0,
        "checkpoint GC must reclaim the aborted insert's version"
    );

    // And it stays invisible.
    let snapshot = engine.begin_transaction().unwrap();
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 0);
    engine.commit_transaction(snapshot).unwrap();
}

#[test]
fn test_gc_keeps_live_snapshots_version_while_reclaiming_aborted() {
    // GC must be surgical: reclaim the aborted insert but keep a committed
    // version a live snapshot can still observe.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 inserts and commits: the visible baseline.
    let t1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.commit_transaction(t1).unwrap();

    // T2 begins: its snapshot observes T1's committed tuple.
    let t2 = engine.begin_transaction().unwrap();

    // T3 inserts and aborts: garbage on the page next to T1's version.
    let t3 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();
    engine.rollback_transaction(t3).unwrap();

    assert_eq!(count_heap_versions(temp_dir.path()), 2);

    // Checkpoint with T2 still live. GC must reclaim exactly one version.
    engine.checkpoint().unwrap();
    assert_eq!(
        count_heap_versions(temp_dir.path()),
        1,
        "GC must reclaim only the aborted version, not T1's"
    );

    // T2 still sees exactly T1's tuple.
    let relation = engine.load_relation_for_txn("TEST", t2.txn_id).unwrap();
    assert_eq!(relation.cardinality(), 1);
    engine.commit_transaction(t2).unwrap();
}
