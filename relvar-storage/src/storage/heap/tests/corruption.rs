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
fn test_heap_update_on_corrupted_page_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // 1. Insert a tuple to get a valid TupleId
    let tuple = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    // 2. Corrupt the page (slot pointing outside)
    // We need to read the page, construct a corrupted version, and write it back
    {
        let page = heap.page_file.read_page(tuple_id.page_id).unwrap();

        // Deserialize (valid)
        let _versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();

        // Create corrupted version
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

        // Serialize and write back
        let slot_dir = postcard::to_allocvec(&corrupted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[0] = PAGE_FORMAT_VERSION;
        let slot_dir_len = slot_dir.len() as u32;
        page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
        heap.page_file.write_page(&new_page).unwrap();
    }

    // 3. Try to update the tuple
    // This should fail when extracting existing tuples in update_tuple_versioned
    let updated = tuple! { id: 1i64, name: "Updated" };
    let result = heap.update_tuple_versioned(tuple_id, &updated, test_txn(2));

    // 4. Assert failure
    assert!(result.is_err(), "Update should fail on corrupted page");
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
fn test_heap_delete_on_corrupted_page_fails() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // 1. Insert a tuple to get a valid TupleId
    let tuple = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

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
        heap.page_file.write_page(&new_page).unwrap();
    }

    // 3. Try to delete the tuple
    let result = heap.delete_tuple_versioned(tuple_id, test_txn(2));

    // 4. Assert failure
    assert!(result.is_err(), "Delete should fail on corrupted page");
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
fn test_sentry_find_page_for_insertion_error_propagation() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let result: Result<(), HeapError> =
        heap.find_page_for_insertion(|_, _| Err(HeapError::TupleTooLarge(10000)));

    assert!(matches!(result, Err(HeapError::TupleTooLarge(10000))));
}

#[test]
fn test_sentry_extract_tuples_from_versioned_slots() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    let mut tuple_data: Vec<u8> = vec![];
    let _ = postcard::to_io(&tuple, &mut tuple_data);

    let mut versioned_page = VersionedSlottedPage {
        magic: 0,
        slot_count: 1,
        slots: vec![Some(VersionedSlotEntry {
            offset: 0,
            length: tuple_data.len() as u32,
            xmin: crate::wal::TransactionId::new(1),
            xmax: None,
            prev_version: None,
        })],
    };

    let existing_tuples = vec![tuple_data.clone()];
    HeapFile::repack_versioned_slots(&mut versioned_page.slots, &existing_tuples, PAGE_SIZE - 8)
        .unwrap();
    let page_data = heap
        .serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)
        .unwrap();
    let page = Page::from_data(0, page_data).unwrap();

    let slots = [versioned_page.slots[0].clone().unwrap()];
    let extracted = heap
        .extract_tuples_from_versioned_slots(&page, slots.iter())
        .unwrap();

    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted[0], tuple);
}

#[test]
fn test_sentry_check_versioned_tuple_size_limit_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let result = heap.check_versioned_tuple_size_limit(usize::MAX - 5, false);
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("Header size + tuple data length overflow")
    ));
}
