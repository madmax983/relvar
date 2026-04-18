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
fn test_delete_sets_xmax() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    // Delete tuple
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

    // Verify xmax is set
    let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_delete_invisible_after_commit() {
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

    // T3: Should not see deleted tuple
    let snapshot_t3 = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
    let visible = heap.scan_visible(&snapshot_t3, &committed).unwrap();

    assert_eq!(visible.len(), 0); // Deleted tuple not visible
}

#[test]
fn test_delete_concurrent_txn_sees_tuple() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Begin (concurrent with T3)
    let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);

    // T3: Delete and commit
    heap.delete_tuple_versioned(tuple_id, test_txn(3)).unwrap();
    committed.insert(test_txn(3));

    // NOTE: With Read Committed, T2 sees the deletion.
    // Full snapshot isolation would preserve visibility.
    let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

    assert_eq!(visible.len(), 0); // Read Committed: sees deletion
}

#[test]
fn test_delete_deleting_txn_doesnt_see_tuple() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Delete
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

    // T2 should not see the tuple it deleted
    let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
    let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

    assert_eq!(visible.len(), 0);
}

#[test]
fn test_delete_nonexistent_tuple_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let bogus_id = TupleId {
        page_id: 999,
        slot: 0,
    };

    let result = heap.delete_tuple_versioned(bogus_id, test_txn(1));
    assert!(result.is_err());
    assert!(matches!(result, Err(HeapError::TupleNotFound)));
}

#[test]
fn test_delete_preserves_xmin() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    // T2: Delete
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

    // Verify xmin unchanged
    let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmin, test_txn(1));
    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_delete_preserves_tuple_data() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let original = tuple! { id: 42i64, name: "DataToPreserve" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Delete tuple
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

    // Tuple data should still be readable (though invisible)
    let tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(tuple, original);
}

#[test]
fn test_delete_already_deleted_sets_xmax_again() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    // Delete by T2
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

    // Delete again by T3 (should succeed and update xmax)
    heap.delete_tuple_versioned(tuple_id, test_txn(3)).unwrap();

    // Verify xmax is now T3
    let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(3)));
}

#[test]
fn test_delete_multiple_tuples() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert multiple tuples
    let t1 = tuple! { id: 1i64, name: "First" };
    let t2 = tuple! { id: 2i64, name: "Second" };
    let t3 = tuple! { id: 3i64, name: "Third" };

    let _tid1 = heap.insert_tuple_versioned(&t1, test_txn(1)).unwrap();
    let tid2 = heap.insert_tuple_versioned(&t2, test_txn(1)).unwrap();
    let _tid3 = heap.insert_tuple_versioned(&t3, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // Delete middle tuple
    heap.delete_tuple_versioned(tid2, test_txn(2)).unwrap();
    committed.insert(test_txn(2));

    // Should see first and third, but not second
    let snapshot = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 2);
    assert!(visible.contains(&t1));
    assert!(!visible.contains(&t2));
    assert!(visible.contains(&t3));
}

#[test]
fn test_delete_and_insert_new_version() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let original = tuple! { id: 1i64, name: "Original" };
    let tid1 = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // Delete
    heap.delete_tuple_versioned(tid1, test_txn(2)).unwrap();
    committed.insert(test_txn(2));

    // Insert new version with same logical key
    let new_ver = tuple! { id: 1i64, name: "Reinserted" };
    heap.insert_tuple_versioned(&new_ver, test_txn(3)).unwrap();
    committed.insert(test_txn(3));

    // Should see only the new version
    let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], new_ver);
}
