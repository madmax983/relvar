#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use relvar_core::values::Tuple;
use std::collections::HashSet;
use tempfile::NamedTempFile;

#[test]
fn test_heap_gc_on_corrupted_page_fails() {
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
    heap.page_file.write_page(&page).unwrap();

    // 2. Run GC
    let result =
        heap.gc_remove_dead_versions(crate::wal::Lsn::new(100), &std::collections::HashSet::new());

    // 3. Assert failure
    assert!(result.is_err(), "GC should fail on corrupted page");
    match result {
        Err(HeapError::Serialization(msg)) => {
            assert!(
                msg.contains("Corrupted slot") || msg.contains("outside page data"),
                "Unexpected error message: {}",
                msg
            );
        }
        _ => panic!("Expected Serialization error, got {:?}", result),
    }
}

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
