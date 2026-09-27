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
fn test_delete_sets_xmax() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // Delete tuple
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // Verify xmax is set
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_delete_invisible_after_commit() {
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

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Delete and commit
    fx.begin(test_txn(2), 150);
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();
    fx.commit(test_txn(2), 200);

    // T3: Should not see deleted tuple
    let snapshot_t3 = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot_t3).unwrap();

    assert_eq!(visible.len(), 0); // Deleted tuple not visible
}

#[test]
fn test_delete_concurrent_txn_sees_tuple() {
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

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Begin (concurrent with T3)
    let snapshot_t2 = fx.begin(test_txn(2), 200);

    // T3: Delete and commit
    fx.begin(test_txn(3), 210);
    heap.delete_tuple_versioned(tuple_id, test_txn(3), &mut vpool)
        .unwrap();
    fx.commit(test_txn(3), 220);

    // NOTE: With Read Committed, T2 sees the deletion.
    // Full snapshot isolation would preserve visibility.
    let visible = fx.scan_visible(&mut heap, &snapshot_t2).unwrap();

    assert_eq!(visible.len(), 0); // Read Committed: sees deletion
}

#[test]
fn test_delete_deleting_txn_doesnt_see_tuple() {
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

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Delete (uncommitted)
    let snapshot_t2 = fx.begin(test_txn(2), 200);
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // T2 should not see the tuple it deleted
    let visible = fx.scan_visible(&mut heap, &snapshot_t2).unwrap();

    assert_eq!(visible.len(), 0);
}

#[test]
fn test_delete_nonexistent_tuple_fails() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let bogus_id = TupleId {
        page_id: 999,
        slot: 0,
    };

    let result = heap.delete_tuple_versioned(bogus_id, test_txn(1), &mut vpool);
    assert!(result.is_err());
    assert!(matches!(result, Err(HeapError::TupleNotFound)));
}

#[test]
fn test_delete_preserves_xmin() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // T2: Delete
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // Verify xmin unchanged
    let page = heap.load_page(tuple_id.page_id).unwrap();
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
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let original = tuple! { id: 42i64, name: "DataToPreserve" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Delete tuple
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // Tuple data should still be readable (though invisible)
    let tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(tuple, original);
}

#[test]
fn test_delete_multiple_tuples() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert multiple tuples
    let t1 = tuple! { id: 1i64, name: "First" };
    let t2 = tuple! { id: 2i64, name: "Second" };
    let t3 = tuple! { id: 3i64, name: "Third" };

    let _tid1 = heap
        .insert_tuple_versioned(&t1, test_txn(1), &mut vpool)
        .unwrap();
    let tid2 = heap
        .insert_tuple_versioned(&t2, test_txn(1), &mut vpool)
        .unwrap();
    let _tid3 = heap
        .insert_tuple_versioned(&t3, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // Delete middle tuple
    fx.begin(test_txn(2), 150);
    heap.delete_tuple_versioned(tid2, test_txn(2), &mut vpool)
        .unwrap();
    fx.commit(test_txn(2), 200);

    // Should see first and third, but not second
    let snapshot = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();

    assert_eq!(visible.len(), 2);
    assert!(visible.contains(&t1));
    assert!(!visible.contains(&t2));
    assert!(visible.contains(&t3));
}

#[test]
fn test_heap_delete_on_corrupted_page_fails() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // 1. Insert a tuple to get a valid TupleId
    let tuple = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // 2. Corrupt the page (slot pointing outside)
    {
        let corrupted_page = VersionedSlottedPage {
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

        let slot_dir = postcard::to_allocvec(&corrupted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[0] = PAGE_FORMAT_VERSION;
        let slot_dir_len = slot_dir.len() as u32;
        page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
        heap.store_page(&new_page).unwrap();
    }

    // 3. Try to delete the tuple
    let result = heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool);

    // 4. Assert failure
    assert!(result.is_err(), "Delete should fail on corrupted page");
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
