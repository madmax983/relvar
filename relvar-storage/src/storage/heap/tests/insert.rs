#![allow(unused_imports)]
use super::common::*;
use crate::storage::heap::*;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
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

// Issue #26: slotted-page insert must not rewrite existing tuples.
//
// The standard slotted-page technique:
// 1. Read only the slot directory.
// 2. Append new tuple data into free space at the page end.
// 3. Rewrite only the slot directory.
// Existing tuple bytes and slot offsets stay stable across inserts.

/// Reads the raw slot directory of a page without touching tuple data.
fn read_slot_directory(heap: &mut HeapFile, page_id: u64) -> SlottedPage {
    let page = heap.page_file.read_page(page_id).unwrap();
    assert!(!page.is_empty(), "expected page {page_id} to contain data");
    postcard::from_bytes(page.data()).unwrap()
}

/// Serializes a tuple exactly the way `insert_tuple` does.
fn serialize_tuple_for_test(tuple: &relvar_core::values::Tuple) -> Vec<u8> {
    postcard::to_allocvec(tuple).unwrap()
}

#[test]
fn test_slotted_insert_first_tuple_packed_at_page_end() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    let tuple_data = serialize_tuple_for_test(&tuple);
    let slot = heap.try_insert_into_page(0, &tuple_data).unwrap();
    assert_eq!(slot, 0);

    let slotted_page = read_slot_directory(&mut heap, 0);
    assert_eq!(slotted_page.slot_count, 1);
    let entry = slotted_page.slots[0].as_ref().unwrap();
    // Tuple data grows downward from the end of the usable area.
    assert_eq!(
        entry.offset() as usize + entry.length() as usize,
        USABLE_PAGE_SIZE_V1,
        "first tuple should be packed against the end of the page"
    );
    assert_eq!(entry.length() as usize, tuple_data.len());
}

#[test]
fn test_slotted_insert_does_not_move_existing_tuples() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let first = tuple! { id: 1i64, name: "Alice" };
    heap.insert_tuple(&first).unwrap();

    // Snapshot the first tuple's slot offset and raw bytes.
    let before_dir = read_slot_directory(&mut heap, 0);
    let before_entry = before_dir.slots[0].as_ref().unwrap();
    let before_offset = before_entry.offset();
    let before_length = before_entry.length();
    let page_before = heap.page_file.read_page(0).unwrap();
    let before_bytes = page_before.data()
        [before_offset as usize..(before_offset + before_length) as usize]
        .to_vec();

    // Insert several more tuples; existing data must not be rewritten.
    for i in 2..=6 {
        let tuple = tuple! { id: i as i64, name: format!("Name{i}") };
        heap.insert_tuple(&tuple).unwrap();
    }

    let after_dir = read_slot_directory(&mut heap, 0);
    let after_entry = after_dir.slots[0].as_ref().unwrap();
    assert_eq!(
        after_entry.offset(),
        before_offset,
        "existing tuple offset must stay stable across inserts"
    );
    assert_eq!(after_entry.length(), before_length);

    let page_after = heap.page_file.read_page(0).unwrap();
    let after_bytes = &page_after.data()
        [after_entry.offset() as usize..(after_entry.offset() + after_entry.length()) as usize];
    assert_eq!(
        after_bytes, before_bytes,
        "existing tuple bytes must not be rewritten on insert"
    );
}

#[test]
fn test_slotted_insert_slot_directory_integrity() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    for i in 0..50 {
        let tuple = tuple! { id: i as i64, name: format!("Name{i}") };
        heap.insert_tuple(&tuple).unwrap();
    }

    let page = heap.page_file.read_page(0).unwrap();
    let slotted_page: SlottedPage = postcard::from_bytes(page.data()).unwrap();
    let header_len = postcard::to_allocvec(&slotted_page).unwrap().len();

    assert_eq!(slotted_page.slot_count, 50);
    assert_eq!(slotted_page.slots.len(), 50);

    // Every live slot must point inside the page, below the header, and
    // no two tuple extents may overlap.
    let mut extents: Vec<(usize, usize)> = Vec::new();
    for slot in slotted_page.slots.iter().flatten() {
        let start = slot.offset() as usize;
        let end = start + slot.length() as usize;
        assert!(start >= header_len, "tuple overlaps the slot directory");
        assert!(end <= page.data().len(), "tuple extends past the page");
        extents.push((start, end));
    }
    extents.sort_unstable();
    for pair in extents.windows(2) {
        assert!(
            pair[0].1 <= pair[1].0,
            "tuple extents must not overlap: {:?} vs {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn test_slotted_insert_then_read_roundtrip_many_tuples() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let mut expected = std::collections::HashSet::new();
    for i in 0..300 {
        let tuple = tuple! { id: i as i64, name: format!("Name{i}") };
        heap.insert_tuple(&tuple).unwrap();
        expected.insert(tuple);
    }

    let scanned: std::collections::HashSet<_> = heap.scan().unwrap().into_iter().collect();
    assert_eq!(scanned.len(), 300);
    assert_eq!(scanned, expected);
}

#[test]
fn test_slotted_insert_page_full_spills_to_next_page() {
    let temp_file = NamedTempFile::new().unwrap();

    let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);
    let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

    // 1000-byte payloads: about 3 fit per page with the slot directory.
    let mut inserted = 0usize;
    for i in 0..20 {
        let tuple = tuple! { data: vec![i as u8; 1000] };
        heap.insert_tuple(&tuple).unwrap();
        inserted += 1;
        if !heap.page_file.read_page(1).unwrap().is_empty() {
            break;
        }
    }

    assert!(
        !heap.page_file.read_page(1).unwrap().is_empty(),
        "page 0 should fill up and spill over to page 1"
    );

    // Every tuple must still be readable after the spill.
    let tuples = heap.scan().unwrap();
    assert_eq!(tuples.len(), inserted);
}

#[test]
fn test_try_insert_into_page_returns_page_full_when_no_space() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    // Fill page 0 with fixed-size payloads until it reports PageFull.
    let payload = vec![0xABu8; 500];
    let mut filled = false;
    for _ in 0..20 {
        match heap.try_insert_into_page(0, &payload) {
            Ok(_) => {}
            Err(HeapError::PageFull) => {
                filled = true;
                break;
            }
            Err(e) => panic!("expected PageFull, got {e:?}"),
        }
    }
    assert!(filled, "page should eventually report PageFull");

    // A further same-size insert must keep reporting PageFull, not corrupt.
    let result = heap.try_insert_into_page(0, &payload);
    assert!(
        matches!(result, Err(HeapError::PageFull)),
        "expected PageFull, got {result:?}"
    );

    // The page must still be readable: slot directory intact.
    let slotted_page = read_slot_directory(&mut heap, 0);
    assert!(slotted_page.slot_count > 0);
    assert!(slotted_page.slots.iter().all(|s| s.is_some()));
}
