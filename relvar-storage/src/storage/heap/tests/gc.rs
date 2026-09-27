#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;

#[test]
fn test_heap_gc_on_corrupted_page_fails() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // 1. Create a corrupted versioned page
    // Slot points to offset > PAGE_SIZE
    let versioned_page = VersionedSlottedPage {
        magic: VERSIONED_PAGE_MAGIC,
        slot_count: 1,
        slots: vec![Some(VersionedSlotEntry {
            offset: (PAGE_SIZE + 100) as u32, // Invalid offset
            length: 10,
            xmin: test_txn(1),
            xmax: None,
            prev_version: None,
        })],
    };

    // Serialize header
    let slot_dir = postcard::to_allocvec(&versioned_page).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE - 8];

    // Write format version
    page_data[0] = PAGE_FORMAT_VERSION;

    // Write slot directory length
    let slot_dir_len = slot_dir.len() as u32;
    page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

    // Copy slot directory after header
    page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

    let page = Page::from_data(0, page_data).unwrap();
    heap.store_page(&page).unwrap();

    // 2. Run GC (fails before the pool is consulted; any pool works)
    let txn_pool = TxnPool::new(4);
    let result = heap.gc_remove_dead_versions(
        crate::wal::Lsn::new(100),
        &std::collections::HashSet::new(),
        &txn_pool,
        &mut vpool,
    );

    // 3. Assert failure
    assert!(result.is_err(), "GC should fail on corrupted page");
    match result {
        Err(HeapError::Slotted(SlottedError::Serialization(msg))) => {
            assert!(
                msg.contains("Corrupted slot") || msg.contains("outside page data"),
                "Unexpected error message: {}",
                msg
            );
        }
        _ => panic!("Expected Slotted error, got {:?}", result),
    }
}

#[test]
fn test_gc_identifies_dead_versions() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete and commit
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();
    committed.insert(test_txn(2));

    // No active transactions: T2's delete committed at LSN 40, well
    // before the oldest active LSN.
    let txn_pool = pool_with_commits(&[(1, 20), (2, 40)]);
    let oldest_active = test_lsn(300);

    // GC should identify and remove the dead version
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 1);
}

#[test]
fn test_gc_preserves_needed_versions() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "Active" };
    heap.insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // Tuple has no xmax (not deleted), should be preserved. The pool is
    // never consulted for versions without xmax.
    let txn_pool = TxnPool::new(4);
    let oldest_active = test_lsn(200);

    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 0); // Nothing removed
}

#[test]
fn test_gc_reclaims_when_oldest_active_reaches_commit_lsn() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit at LSN 100
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete and commit at LSN 200
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();
    committed.insert(test_txn(2));

    // Boundary: the oldest active LSN is exactly the deleter's commit
    // LSN. Every live snapshot started at or after the commit, so none
    // can observe the old version — it is dead (`commit_lsn <=
    // oldest_active_lsn`).
    let txn_pool = pool_with_commits(&[(1, 100), (2, 200)]);
    let oldest_active = test_lsn(200);

    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 1);
}

#[test]
fn test_gc_uncommitted_xmax_preserved() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete but NOT committed
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();
    // T2 not in committed set. The pool is never consulted for
    // uncommitted deleters.
    let txn_pool = TxnPool::new(4);

    let oldest_active = test_lsn(300);

    // Should NOT remove (xmax not committed)
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 0);
}

#[test]
fn test_gc_multiple_tuples() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert 3 tuples, delete 2
    let t1 = tuple! { id: 1i64, name: "One" };
    let t2 = tuple! { id: 2i64, name: "Two" };
    let t3 = tuple! { id: 3i64, name: "Three" };

    let tid1 = heap
        .insert_tuple_versioned(&t1, test_txn(1), &mut vpool)
        .unwrap();
    let tid2 = heap
        .insert_tuple_versioned(&t2, test_txn(1), &mut vpool)
        .unwrap();
    heap.insert_tuple_versioned(&t3, test_txn(1), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // Delete first two
    heap.delete_tuple_versioned(tid1, test_txn(2), &mut vpool)
        .unwrap();
    heap.delete_tuple_versioned(tid2, test_txn(2), &mut vpool)
        .unwrap();
    committed.insert(test_txn(2));

    // Both deletes committed at LSN 40, before the oldest active LSN.
    let txn_pool = pool_with_commits(&[(1, 20), (2, 40)]);
    let oldest_active = test_lsn(300);

    // Should remove 2 versions
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 2);
}

#[test]
fn test_gc_empty_heap() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let committed = HashSet::new();
    let txn_pool = TxnPool::new(4);
    let oldest_active = test_lsn(100);

    // GC on empty heap should succeed
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 0);
}

#[test]
fn test_gc_preserves_live_versions_after_gc() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let old = tuple! { id: 1i64, name: "Old" };
    let tid_old = heap
        .insert_tuple_versioned(&old, test_txn(1), &mut vpool)
        .unwrap();

    // T2: Update
    let new = tuple! { id: 1i64, name: "New" };
    heap.update_tuple_versioned(tid_old, &new, test_txn(2), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);
    fx.begin(test_txn(2), 150);
    fx.commit(test_txn(2), 200);

    // GC old version. The fixture's pool records T2's commit at LSN 200,
    // before the oldest active LSN, so the old version is dead.
    let oldest_active = test_lsn(300);
    heap.gc_remove_dead_versions(oldest_active, &fx.committed, &fx.pool, &mut vpool)
        .unwrap();

    // New version should still be visible
    let snapshot = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], new);
}

#[test]
fn test_gc_counts_removed_correctly() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create multiple dead versions
    for i in 1..=5 {
        let tuple = tuple! { id: i, name: "Test" };
        let tid = heap
            .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
            .unwrap();
        heap.delete_tuple_versioned(tid, test_txn(2), &mut vpool)
            .unwrap();
    }

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));

    // Both deletes committed at LSN 40, before the oldest active LSN.
    let txn_pool = pool_with_commits(&[(1, 20), (2, 40)]);
    let oldest_active = test_lsn(300);
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 5);
}

#[test]
fn test_gc_idempotent() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert and delete
    let tuple = tuple! { id: 1i64, name: "Test" };
    let tid = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    heap.delete_tuple_versioned(tid, test_txn(2), &mut vpool)
        .unwrap();
    committed.insert(test_txn(2));

    let oldest_active = test_lsn(300);

    // The delete committed at LSN 40, before the oldest active LSN.
    let txn_pool = pool_with_commits(&[(1, 20), (2, 40)]);

    // First GC
    let removed1 = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();
    assert_eq!(removed1, 1);

    // Second GC should remove nothing
    let removed2 = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();
    assert_eq!(removed2, 0);
}

use crate::mvcc::TxnPool;
use crate::wal::TransactionId;

#[test]
fn test_gc_keeps_version_visible_to_live_snapshot() {
    // Regression test: GC used to decide death by comparing the deleting
    // transaction's ID against the oldest active LSN (`xmax <
    // oldest_active_lsn`). IDs and LSNs are different sequences — IDs stay
    // tiny while the LSN grows with every WAL record — so in any
    // long-lived database the comparison was true for every deleter, and
    // GC reclaimed versions a live snapshot could still observe. Death
    // must be decided from the deleter's *commit LSN*, resolved through
    // the transaction pool.
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1 inserts; T2 deletes. Small IDs, realistic LSNs.
    let tuple = tuple! { id: 1i64, name: "Doomed" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut txn_pool = TxnPool::new(8);
    let horizon = TransactionId::new(u64::MAX);
    txn_pool.begin(test_txn(1), test_lsn(10), horizon).unwrap();
    txn_pool.commit(test_txn(1), test_lsn(20)).unwrap();

    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();
    txn_pool.begin(test_txn(2), test_lsn(30), horizon).unwrap();
    txn_pool.commit(test_txn(2), test_lsn(200)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));

    // A live snapshot taken at LSN 150 — before T2's delete committed at
    // LSN 200 — can still observe the old version. GC must keep it, even
    // though the deleter's ID (2) is far below the oldest active LSN.
    let removed = heap
        .gc_remove_dead_versions(test_lsn(150), &committed, &txn_pool, &mut vpool)
        .unwrap();
    assert_eq!(
        removed, 0,
        "GC must not reclaim a version visible to a live snapshot"
    );

    // Once the oldest live snapshot starts at or after the delete's commit
    // LSN, no snapshot can observe the old version: GC reclaims it.
    let removed = heap
        .gc_remove_dead_versions(test_lsn(200), &committed, &txn_pool, &mut vpool)
        .unwrap();
    assert_eq!(
        removed, 1,
        "GC must reclaim the version once no snapshot can observe it"
    );
}

#[test]
fn test_gc_reclaims_aborted_xmin_version() {
    // A version created by an aborted transaction has xmax=None forever,
    // so the deleter-based rule can never catch it. The creator-death rule
    // (not committed, not live) must reclaim it.
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let tuple = tuple! { id: 1i64, name: "Aborted" };
    let mut fx = MvccFixture::new();
    fx.begin(test_txn(5), 10);
    heap.insert_tuple_versioned(&tuple, test_txn(5), &mut vpool)
        .unwrap();
    fx.pool.abort(test_txn(5)).unwrap();

    let removed = heap
        .gc_remove_dead_versions(test_lsn(300), &fx.committed, &fx.pool, &mut vpool)
        .unwrap();
    assert_eq!(
        removed, 1,
        "GC must reclaim the aborted transaction's version"
    );

    // The reclaimed version stays invisible: a fresh snapshot sees nothing.
    let snapshot = fx.begin(test_txn(6), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();
    assert!(visible.is_empty(), "aborted version must stay invisible");
}

#[test]
fn test_gc_keeps_uncommitted_version_of_live_txn() {
    // The mirror image of the abort case: a version whose creator is
    // uncommitted but STILL LIVE may yet commit, so GC must keep it even
    // though it is invisible to every current snapshot.
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let tuple = tuple! { id: 1i64, name: "InFlight" };
    let mut fx = MvccFixture::new();
    fx.begin(test_txn(5), 10);
    heap.insert_tuple_versioned(&tuple, test_txn(5), &mut vpool)
        .unwrap();
    // T5 is neither committed nor aborted — still live.

    let removed = heap
        .gc_remove_dead_versions(test_lsn(300), &fx.committed, &fx.pool, &mut vpool)
        .unwrap();
    assert_eq!(
        removed, 0,
        "GC must keep versions of a still-live transaction"
    );

    // Its own snapshot still sees its write.
    let snapshot = fx.pool.get_snapshot(test_txn(5)).unwrap();
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], tuple);
}

#[test]
fn test_gc_reclaims_only_aborted_among_mixed_versions() {
    // Mixed page: an aborted insert, a live transaction's uncommitted
    // insert, and a committed insert. GC must reclaim exactly the aborted
    // one and leave the other two observable by the right snapshots.
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let mut fx = MvccFixture::new();

    let aborted_tuple = tuple! { id: 1i64, name: "Aborted" };
    fx.begin(test_txn(5), 10);
    heap.insert_tuple_versioned(&aborted_tuple, test_txn(5), &mut vpool)
        .unwrap();
    fx.pool.abort(test_txn(5)).unwrap();

    let live_tuple = tuple! { id: 2i64, name: "Live" };
    fx.begin(test_txn(6), 20);
    heap.insert_tuple_versioned(&live_tuple, test_txn(6), &mut vpool)
        .unwrap();

    let committed_tuple = tuple! { id: 3i64, name: "Committed" };
    fx.begin(test_txn(7), 30);
    heap.insert_tuple_versioned(&committed_tuple, test_txn(7), &mut vpool)
        .unwrap();
    fx.commit(test_txn(7), 40);

    let removed = heap
        .gc_remove_dead_versions(test_lsn(300), &fx.committed, &fx.pool, &mut vpool)
        .unwrap();
    assert_eq!(removed, 1, "GC must reclaim exactly the aborted version");

    // Second pass is idempotent.
    let removed = heap
        .gc_remove_dead_versions(test_lsn(300), &fx.committed, &fx.pool, &mut vpool)
        .unwrap();
    assert_eq!(removed, 0);

    // A fresh snapshot sees the committed tuple and neither the aborted
    // nor the other transaction's uncommitted one.
    let snapshot = fx.begin(test_txn(8), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], committed_tuple);

    // The live transaction still sees its own write plus the committed one.
    let snapshot = fx.pool.get_snapshot(test_txn(6)).unwrap();
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();
    assert_eq!(visible.len(), 2);
    assert!(visible.contains(&live_tuple));
    assert!(visible.contains(&committed_tuple));
}
