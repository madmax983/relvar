#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use tempfile::NamedTempFile;

#[test]
fn test_heap_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    let rel_type = create_test_relation_type();

    // Write tuples
    {
        let mut heap = HeapFile::create(&path, rel_type.clone()).unwrap();
        heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
            .unwrap();
    }

    // Read them back in a new instance
    {
        let mut heap = HeapFile::open(&path, rel_type.clone()).unwrap();
        let results = heap.scan().unwrap();
        assert_eq!(results.len(), 2);
    }
}

// test_tuple_not_found removed - tested internal read_tuple with TupleId which is now pub(crate)

// Tests verifying TupleId is not exposed in public APIs (TTM Proscription 6)

#[test]
fn test_heap_page_growth() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create a tuple that takes up roughly half a page (2000 bytes)
    // Page size is 4096. Overhead is small.
    // 2 tuples should fit. 3rd should force new page.
    let data = vec![0u8; 2000];
    let tuple = tuple! { data: data };

    heap.insert_tuple(&tuple).unwrap(); // Page 0
    heap.insert_tuple(&tuple).unwrap(); // Page 0 (should fit, 4000 < 4096 - overhead)

    // Let's make it larger to guarantee split. 3000 bytes.
    let data_large = vec![0u8; 3000];
    let tuple_large = tuple! { data: data_large };

    // Re-create heap to start fresh
    let mut heap = HeapFile::create(path, create_test_relation_type()).unwrap(); // Reset

    // 1st tuple: 3000 bytes. Page 0.
    heap.insert_tuple(&tuple_large).unwrap();

    // 2nd tuple: 3000 bytes. Should go to Page 1.
    // If my fix broke page creation, this will fail (err instead of new page).
    heap.insert_tuple(&tuple_large).unwrap();

    let tuples = heap.scan().unwrap();
    assert_eq!(tuples.len(), 2);
}

// MVCC versioned slot entry tests

#[allow(dead_code)]
fn test_txn(value: u64) -> crate::wal::TransactionId {
    crate::wal::TransactionId::new(value)
}

#[test]
fn test_sentry_extract_tuples_from_slots() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    let mut tuple_data: Vec<u8> = vec![];
    postcard::to_io(&tuple, &mut tuple_data).unwrap();

    let mut slotted_page = SlottedPage {
        slot_count: 1,
        slots: vec![Some(SlotEntry {
            offset: 0,
            length: tuple_data.len() as u32,
        })],
    };

    let existing_tuples = vec![tuple_data.clone()];
    // Using a very simplified repack matching logic
    let usable_size = PAGE_SIZE - 8;
    slotted_page.slots[0].as_mut().unwrap().offset = (usable_size - tuple_data.len()) as u32;

    let page_data = heap
        .serialize_slotted_page_with_tuples(&slotted_page, &existing_tuples)
        .unwrap();
    let page = Page::from_data(0, page_data).unwrap();

    let slots = [slotted_page.slots[0].clone().unwrap()];

    let extracted = heap.extract_tuples_from_slots(&page, slots.iter()).unwrap();

    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted[0], tuple);
}

#[test]
fn test_sentry_extract_all_tuples() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let tuple1 = tuple! { id: 1i64, name: "Alice" };
    let mut tuple_data1: Vec<u8> = vec![];
    postcard::to_io(&tuple1, &mut tuple_data1).unwrap();

    let mut slotted_page = SlottedPage {
        slot_count: 2,
        slots: vec![
            Some(SlotEntry {
                offset: 0,
                length: tuple_data1.len() as u32,
            }),
            None,
        ],
    };

    let existing_tuples = vec![tuple_data1.clone(), vec![]];
    let usable_size = PAGE_SIZE - 8;
    slotted_page.slots[0].as_mut().unwrap().offset = (usable_size - tuple_data1.len()) as u32;

    let page_data = heap
        .serialize_slotted_page_with_tuples(&slotted_page, &existing_tuples)
        .unwrap();
    let page = Page::from_data(0, page_data).unwrap();

    let extracted = heap.extract_all_tuples(&page, &slotted_page.slots).unwrap();

    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0], tuple_data1);
    assert_eq!(extracted[1].len(), 0);
}
