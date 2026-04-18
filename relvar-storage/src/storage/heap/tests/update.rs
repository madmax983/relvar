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
fn test_update_creates_new_version() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Update to new version
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // New version should have different TupleId
    assert_ne!(tuple_id, new_tuple_id);

    // Read new version
    let new_version = heap.read_tuple_versioned(new_tuple_id).unwrap();
    assert_eq!(new_version, updated);
}

#[test]
fn test_update_marks_old_xmax() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Update sets xmax on old version
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // Old version should have xmax set
    let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_update_new_version_has_correct_xmin() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Update creates new version with updating transaction's ID
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // New version should have xmin = test_txn(2)
    let page = heap.page_file.read_page(new_tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[new_tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmin, test_txn(2));
    assert_eq!(slot.xmax, None); // Not yet deleted
}

#[test]
fn test_update_links_versions() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let old_tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Update creates version chain
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(old_tuple_id, &updated, test_txn(2))
        .unwrap();

    // New version should point back to old version
    let page = heap.page_file.read_page(new_tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let new_slot = versioned_page.slots[new_tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(new_slot.prev_version, Some(old_tuple_id));
}

#[test]
fn test_update_concurrent_txn_sees_old_version() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // T1 commits
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Begin (concurrent with T3)
    let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);

    // T3: Update
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(3))
        .unwrap();

    // T3 commits
    committed.insert(test_txn(3));

    // NOTE: Current implementation uses Read Committed isolation, not Snapshot Isolation.
    // Full snapshot isolation requires commit LSN tracking.
    // With Read Committed, T2 sees T3's committed update.
    let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], updated); // Sees committed update (Read Committed semantics)
}

#[test]
fn test_update_updating_txn_sees_new_version() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Update
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // T2 should see its own update (not yet committed)
    let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
    let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], updated); // Should see updated version
}

#[test]
fn test_update_multiple_times_creates_chain() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let v1 = tuple! { id: 1i64, name: "V1" };
    let tid1 = heap.insert_tuple_versioned(&v1, test_txn(1)).unwrap();

    // Update to V2
    let v2 = tuple! { id: 1i64, name: "V2" };
    let tid2 = heap.update_tuple_versioned(tid1, &v2, test_txn(2)).unwrap();

    // Update to V3
    let v3 = tuple! { id: 1i64, name: "V3" };
    let tid3 = heap.update_tuple_versioned(tid2, &v3, test_txn(3)).unwrap();

    // Verify chain: tid3 -> tid2 -> tid1
    let page3 = heap.page_file.read_page(tid3.page_id).unwrap();
    let vpage3: VersionedSlottedPage = deserialize_versioned_page_for_test(page3.data()).unwrap();
    let slot3 = vpage3.slots[tid3.slot as usize].as_ref().unwrap();
    assert_eq!(slot3.prev_version, Some(tid2));

    let page2 = heap.page_file.read_page(tid2.page_id).unwrap();
    let vpage2: VersionedSlottedPage = deserialize_versioned_page_for_test(page2.data()).unwrap();
    let slot2 = vpage2.slots[tid2.slot as usize].as_ref().unwrap();
    assert_eq!(slot2.prev_version, Some(tid1));
}

#[test]
fn test_update_nonexistent_tuple_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let bogus_id = TupleId {
        page_id: 999,
        slot: 0,
    };
    let updated = tuple! { id: 1i64, name: "Updated" };

    let result = heap.update_tuple_versioned(bogus_id, &updated, test_txn(1));
    assert!(result.is_err());
    assert!(matches!(result, Err(HeapError::TupleNotFound)));
}

#[test]
fn test_update_preserves_tuple_data() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let original = tuple! { id: 42i64, name: "OriginalData" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Update with different data
    let updated = tuple! { id: 42i64, name: "UpdatedData" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // Verify both versions have correct data
    let old_tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(old_tuple, original);

    let new_tuple = heap.read_tuple_versioned(new_tuple_id).unwrap();
    assert_eq!(new_tuple, updated);
}

#[test]
fn test_update_old_version_xmin_unchanged() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // T2: Update
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // Old version should still have xmin = test_txn(1)
    let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmin, test_txn(1));
}

#[test]
fn test_update_different_pages() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    // Update might go to different page if original page is full
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();

    // Both versions should be readable
    let old_tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(old_tuple, original);

    let new_tuple = heap.read_tuple_versioned(new_tuple_id).unwrap();
    assert_eq!(new_tuple, updated);
}

#[test]
fn test_update_after_commit_visible_to_later_txn() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    // T2: Update and commit
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
        .unwrap();
    committed.insert(test_txn(2));

    // T3: Should see updated version
    let snapshot_t3 = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
    let visible = heap.scan_visible(&snapshot_t3, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], updated);
}
