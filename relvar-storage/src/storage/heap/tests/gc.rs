#![allow(unused_imports)]
use crate::storage::heap::*;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;
use crate::wal::{Lsn, TransactionId};
use crate::mvcc::TransactionSnapshot;
use super::common::*;

#[test]
fn test_gc_identifies_dead_versions() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete and commit
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();
    committed.insert(test_txn(2));

    // No active transactions (oldest_active_lsn after T2)
    let oldest_active = test_lsn(300);

    // GC should identify and remove the dead version
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 1);
}

#[test]
fn test_gc_preserves_needed_versions() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "Active" };
    heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // Tuple has no xmax (not deleted), should be preserved
    let oldest_active = test_lsn(200);

    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 0); // Nothing removed
}

#[test]
fn test_gc_respects_oldest_active() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete and commit at LSN 200
    heap.delete_tuple_versioned(tuple_id, test_txn(200))
        .unwrap();
    committed.insert(test_txn(200));

    // Active transaction started before T2 deleted (LSN 150 < 200)
    // NOTE: Without commit LSN tracking, we use txn ID as proxy for LSN
    // GC should NOT remove (might still be visible to older snapshot)
    let oldest_active = test_lsn(150);

    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 0); // Nothing removed
}

#[test]
fn test_gc_removes_entire_update_chain() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let v1 = tuple! { id: 1i64, name: "V1" };
    let tid1 = heap.insert_tuple_versioned(&v1, test_txn(1)).unwrap();

    // T2: Update to V2
    let v2 = tuple! { id: 1i64, name: "V2" };
    let tid2 = heap.update_tuple_versioned(tid1, &v2, test_txn(2)).unwrap();

    // T3: Update to V3
    let v3 = tuple! { id: 1i64, name: "V3" };
    let _tid3 = heap.update_tuple_versioned(tid2, &v3, test_txn(3)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));
    committed.insert(test_txn(3));

    // All transactions committed, no active txns
    let oldest_active = test_lsn(400);

    // Should remove V1 and V2 (both have xmax and are old)
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 2);
}

#[test]
fn test_gc_uncommitted_xmax_preserved() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete but NOT committed
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();
    // T2 not in committed set

    let oldest_active = test_lsn(300);

    // Should NOT remove (xmax not committed)
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 0);
}

#[test]
fn test_gc_multiple_tuples() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert 3 tuples, delete 2
    let t1 = tuple! { id: 1i64, name: "One" };
    let t2 = tuple! { id: 2i64, name: "Two" };
    let t3 = tuple! { id: 3i64, name: "Three" };

    let tid1 = heap.insert_tuple_versioned(&t1, test_txn(1)).unwrap();
    let tid2 = heap.insert_tuple_versioned(&t2, test_txn(1)).unwrap();
    heap.insert_tuple_versioned(&t3, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // Delete first two
    heap.delete_tuple_versioned(tid1, test_txn(2)).unwrap();
    heap.delete_tuple_versioned(tid2, test_txn(2)).unwrap();
    committed.insert(test_txn(2));

    let oldest_active = test_lsn(300);

    // Should remove 2 versions
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 2);
}

#[test]
fn test_gc_empty_heap() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let committed = HashSet::new();
    let oldest_active = test_lsn(100);

    // GC on empty heap should succeed
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 0);
}

#[test]
fn test_gc_preserves_live_versions_after_gc() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let old = tuple! { id: 1i64, name: "Old" };
    let tid_old = heap.insert_tuple_versioned(&old, test_txn(1)).unwrap();

    // T2: Update
    let new = tuple! { id: 1i64, name: "New" };
    heap.update_tuple_versioned(tid_old, &new, test_txn(2))
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));

    // GC old version
    let oldest_active = test_lsn(300);
    heap.gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    // New version should still be visible
    let snapshot = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], new);
}

#[test]
fn test_gc_counts_removed_correctly() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create multiple dead versions
    for i in 1..=5 {
        let tuple = tuple! { id: i, name: "Test" };
        let tid = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();
        heap.delete_tuple_versioned(tid, test_txn(2)).unwrap();
    }

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));

    let oldest_active = test_lsn(300);
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();

    assert_eq!(removed, 5);
}

#[test]
fn test_gc_idempotent() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert and delete
    let tuple = tuple! { id: 1i64, name: "Test" };
    let tid = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    heap.delete_tuple_versioned(tid, test_txn(2)).unwrap();
    committed.insert(test_txn(2));

    let oldest_active = test_lsn(300);

    // First GC
    let removed1 = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();
    assert_eq!(removed1, 1);

    // Second GC should remove nothing
    let removed2 = heap
        .gc_remove_dead_versions(oldest_active, &committed)
        .unwrap();
    assert_eq!(removed2, 0);
}
