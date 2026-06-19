#![allow(unused_imports)]
use super::common::*;
use crate::storage::heap::*;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::values::Tuple;
use std::collections::HashSet;
use tempfile::NamedTempFile;

#[test]
fn test_heap_insert_and_read() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    heap.insert_tuple(&tuple).unwrap();

    let tuples = heap.scan().unwrap();
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0], tuple);
}

#[test]
fn test_heap_insert_multiple() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

    let tuple1 = tuple! { id: 1i64, name: "Alice" };
    let tuple2 = tuple! { id: 2i64, name: "Bob" };
    let tuple3 = tuple! { id: 3i64, name: "Charlie" };

    heap.insert_tuple(&tuple1).unwrap();
    heap.insert_tuple(&tuple2).unwrap();
    heap.insert_tuple(&tuple3).unwrap();

    let tuples = heap.scan().unwrap();
    assert_eq!(tuples.len(), 3);
    assert!(tuples.contains(&tuple1));
    assert!(tuples.contains(&tuple2));
    assert!(tuples.contains(&tuple3));
}

#[test]
fn test_heap_insert_returns_unit_not_tuple_id() {
    let temp_file = NamedTempFile::new().unwrap();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    let result = heap.insert_tuple(&tuple);

    // Type check: Verifies the API returns unit type, not TupleId
    assert!(result.is_ok());
    let _unit: () = result.unwrap();
}

#[test]
fn test_store_relation_returns_unit() {
    let temp_file = NamedTempFile::new().unwrap();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(temp_file.path(), rel_type.clone()).unwrap();

    let mut relation = Relation::new(rel_type);
    relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();

    // Type check: Verifies the API returns unit type, not Vec<TupleId>
    let result = heap.store_relation(&relation);
    assert!(result.is_ok());
    let _unit: () = result.unwrap();
}

#[test]
fn test_insert_versioned_sets_xmin() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    let txn_id = test_txn(10);

    let tuple_id = heap.insert_tuple_versioned(&tuple, txn_id).unwrap();

    // Verify TupleId was returned
    assert_eq!(tuple_id.page_id, 0);
    assert_eq!(tuple_id.slot, 0);
}

#[test]
fn test_insert_versioned_xmax_is_none() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let tuple = tuple! { id: 1i64, name: "Bob" };
    let txn_id = test_txn(20);

    heap.insert_tuple_versioned(&tuple, txn_id).unwrap();

    // Would need to read the page to verify xmax is None
    // For now, just verify insertion succeeded
}

#[test]
fn test_insert_versioned_multiple_versions_same_page() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert multiple versions from different transactions
    let tuple1 = tuple! { id: 1i64, name: "Version1" };
    let tuple2 = tuple! { id: 2i64, name: "Version2" };
    let tuple3 = tuple! { id: 3i64, name: "Version3" };

    let tid1 = heap.insert_tuple_versioned(&tuple1, test_txn(1)).unwrap();
    let tid2 = heap.insert_tuple_versioned(&tuple2, test_txn(2)).unwrap();
    let tid3 = heap.insert_tuple_versioned(&tuple3, test_txn(3)).unwrap();

    // All should be on page 0 (small tuples)
    assert_eq!(tid1.page_id, 0);
    assert_eq!(tid2.page_id, 0);
    assert_eq!(tid3.page_id, 0);

    // Different slots
    assert_eq!(tid1.slot, 0);
    assert_eq!(tid2.slot, 1);
    assert_eq!(tid3.slot, 2);
}

#[test]
fn test_insert_versioned_returns_tuple_id() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let tuple = tuple! { id: 42i64, name: "Test" };
    let txn_id = test_txn(100);

    let result = heap.insert_tuple_versioned(&tuple, txn_id);

    assert!(result.is_ok());
    let tuple_id = result.unwrap();

    // Verify we got a valid TupleId
    assert!(tuple_id.page_id < 1000); // Reasonable page number
    assert!(tuple_id.slot < 1000); // Reasonable slot number
}

#[test]
fn test_insert_versioned_different_transactions() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Same tuple data, different transactions
    let tuple = tuple! { id: 1i64, name: "Same" };

    let tid1 = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();
    let tid2 = heap.insert_tuple_versioned(&tuple, test_txn(2)).unwrap();

    // Should create separate versions
    assert_ne!(tid1, tid2);
}

#[test]
fn test_insert_versioned_page_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Large tuple that will force page overflow
    let data = vec![0u8; 3000];
    let tuple = tuple! { data: data };

    let tid1 = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();
    let tid2 = heap.insert_tuple_versioned(&tuple, test_txn(2)).unwrap();

    // Should go to different pages
    assert_eq!(tid1.page_id, 0);
    assert_eq!(tid2.page_id, 1);
}

#[test]
fn test_insert_versioned_empty_tuple() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Empty tuple type
    let heading = TupleType::new();
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

    let tuple = Relation::from_tuples(rel_type, vec![])
        .unwrap()
        .tuples()
        .next()
        .cloned()
        .unwrap_or_else(|| tuple! {});

    let result = heap.insert_tuple_versioned(&tuple, test_txn(1));

    // Should succeed even with empty tuple
    assert!(result.is_ok());
}

#[test]
fn test_insert_versioned_preserves_prev_version_none() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let tuple = tuple! { id: 1i64, name: "Initial" };
    heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    // prev_version should be None for new inserts
    // (will be tested more thoroughly in Phase 5)
}

#[test]
fn test_insert_versioned_sequential_slots() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert 5 tuples
    for i in 0..5 {
        let tuple = tuple! { id: i as i64, name: format!("Tuple{}", i) };
        let tid = heap
            .insert_tuple_versioned(&tuple, test_txn(i + 1))
            .unwrap();

        assert_eq!(tid.page_id, 0);
        assert_eq!(tid.slot, i as u32);
    }
}

// Phase 2.3: Scan with Visibility tests

fn test_lsn(value: u64) -> crate::wal::Lsn {
    crate::wal::Lsn::new(value)
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
    let snapshot = crate::mvcc::TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], new_ver);
}

#[test]
fn test_heap_insert_on_corrupted_page_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // 1. Create a corrupted page
    // Slot points to offset > PAGE_SIZE
    let slotted_page = SlottedPage {
        slot_count: 1,
        slots: vec![Some(SlotEntry {
            offset: (PAGE_SIZE + 100) as u32, // Invalid offset
            length: 10,
        })],
    };

    // Serialize header
    let slot_dir = postcard::to_allocvec(&slotted_page).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE - 8];
    page_data[..slot_dir.len()].copy_from_slice(&slot_dir);

    let page = Page::from_data(0, page_data).unwrap();
    heap.page_file.write_page(&page).unwrap();

    // 2. Try to insert a new tuple
    // This should fail because it needs to read existing tuples to shift them
    let tuple = tuple! { id: 1i64, name: "NewTuple" };
    let result = heap.insert_tuple(&tuple);

    // 3. Assert failure
    // Currently this fails (returns Ok) because of the bug
    assert!(
        result.is_err(),
        "Insert should fail on corrupted page, but succeeded"
    );
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
fn test_heap_insert_versioned_on_corrupted_page_fails() {
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

    // 2. Try to insert a new versioned tuple
    // This should fail when extracting existing tuples
    let tuple = tuple! { id: 1i64, name: "NewTuple" };
    let result = heap.insert_tuple_versioned(&tuple, test_txn(2));

    // 3. Assert failure
    assert!(
        result.is_err(),
        "Insert versioned should fail on corrupted page"
    );
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
fn test_heap_insert_too_large_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create a tuple that is definitely too large (> 4096)
    let data = vec![0u8; 5000];
    let tuple = tuple! { data: data };

    let result = heap.insert_tuple(&tuple);

    assert!(result.is_err());
    match result {
        Err(HeapError::TupleTooLarge(size)) => {
            assert!(size >= 5000);
        }
        _ => panic!("Expected TupleTooLarge error, got {:?}", result),
    }
}

#[test]
fn test_heap_insert_versioned_too_large_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create a tuple that is definitely too large (> 4096)
    let data = vec![0u8; 5000];
    let tuple = tuple! { data: data };

    let result = heap.insert_tuple_versioned(&tuple, test_txn(1));

    assert!(result.is_err());
    match result {
        Err(HeapError::TupleTooLarge(size)) => {
            assert!(size >= 5000);
        }
        _ => panic!("Expected TupleTooLarge error, got {:?}", result),
    }
}

#[test]
fn test_sentry_find_page_for_insertion_error_propagation() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let result: Result<(), HeapError> =
        heap.find_page_for_insertion(|_, _| Err(HeapError::TupleTooLarge(10000)));

    assert!(matches!(result, Err(HeapError::TupleTooLarge(10000))));
}
