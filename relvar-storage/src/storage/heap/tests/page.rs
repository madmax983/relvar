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
fn test_verify_versioned_page_size_overflow() {
    let slot_dir = vec![0; 100];
    let header_size = usize::MAX;
    let usable_size = 1000;

    let result = HeapFile::verify_versioned_page_size(&slot_dir, header_size, usable_size);

    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("Header size overflow")
    ));
}

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

#[test]
fn test_versioned_slot_serialization() {
    let entry = VersionedSlotEntry {
        offset: 100,
        length: 50,
        xmin: test_txn(1),
        xmax: None,
        prev_version: None,
    };

    // Serialize and deserialize
    let serialized = postcard::to_allocvec(&entry).unwrap();
    let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();

    assert_eq!(entry, deserialized);
}

#[test]
fn test_versioned_slot_with_xmax() {
    let entry = VersionedSlotEntry {
        offset: 200,
        length: 75,
        xmin: test_txn(1),
        xmax: Some(test_txn(2)),
        prev_version: None,
    };

    let serialized = postcard::to_allocvec(&entry).unwrap();
    let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();

    assert_eq!(entry, deserialized);
    assert_eq!(deserialized.xmax, Some(test_txn(2)));
}

#[test]
fn test_versioned_slot_with_prev_version() {
    let prev = TupleId {
        page_id: 5,
        slot: 10,
    };

    let entry = VersionedSlotEntry {
        offset: 300,
        length: 100,
        xmin: test_txn(3),
        xmax: Some(test_txn(4)),
        prev_version: Some(prev),
    };

    let serialized = postcard::to_allocvec(&entry).unwrap();
    let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();

    assert_eq!(entry, deserialized);
    assert_eq!(deserialized.prev_version, Some(prev));
}

#[test]
fn test_versioned_page_layout() {
    let slots = vec![
        Some(VersionedSlotEntry {
            offset: 4000,
            length: 50,
            xmin: test_txn(1),
            xmax: None,
            prev_version: None,
        }),
        Some(VersionedSlotEntry {
            offset: 3900,
            length: 80,
            xmin: test_txn(2),
            xmax: Some(test_txn(3)),
            prev_version: None,
        }),
        None, // Empty slot (deleted tuple)
    ];

    let page = VersionedSlottedPage {
        magic: VERSIONED_PAGE_MAGIC,
        slot_count: 3,
        slots,
    };

    // Serialize and deserialize
    let serialized = postcard::to_allocvec(&page).unwrap();
    let deserialized: VersionedSlottedPage = postcard::from_bytes(&serialized).unwrap();

    assert_eq!(page.slot_count, deserialized.slot_count);
    assert_eq!(page.slots.len(), deserialized.slots.len());

    // Verify first slot
    assert_eq!(
        page.slots[0].as_ref().unwrap().xmin,
        deserialized.slots[0].as_ref().unwrap().xmin
    );

    // Verify second slot has xmax
    assert_eq!(page.slots[1].as_ref().unwrap().xmax, Some(test_txn(3)));

    // Verify third slot is None
    assert!(deserialized.slots[2].is_none());
}

#[test]
fn test_versioned_slot_roundtrip_multiple() {
    // Test multiple entries with various combinations
    let entries = vec![
        VersionedSlotEntry {
            offset: 1000,
            length: 100,
            xmin: test_txn(1),
            xmax: None,
            prev_version: None,
        },
        VersionedSlotEntry {
            offset: 2000,
            length: 200,
            xmin: test_txn(2),
            xmax: Some(test_txn(3)),
            prev_version: Some(TupleId {
                page_id: 0,
                slot: 0,
            }),
        },
        VersionedSlotEntry {
            offset: 3000,
            length: 300,
            xmin: test_txn(4),
            xmax: Some(test_txn(5)),
            prev_version: Some(TupleId {
                page_id: 1,
                slot: 1,
            }),
        },
    ];

    for entry in entries {
        let serialized = postcard::to_allocvec(&entry).unwrap();
        let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();
        assert_eq!(entry, deserialized);
    }
}

#[test]
fn test_versioned_page_empty_slots() {
    let page = VersionedSlottedPage {
        magic: VERSIONED_PAGE_MAGIC,
        slot_count: 0,
        slots: Vec::new(),
    };

    let serialized = postcard::to_allocvec(&page).unwrap();
    let deserialized: VersionedSlottedPage = postcard::from_bytes(&serialized).unwrap();

    assert_eq!(page.slot_count, deserialized.slot_count);
    assert_eq!(page.slots.len(), 0);
}

#[test]
fn test_versioned_slot_size_vs_regular() {
    // Ensure we understand the size increase
    let regular = SlotEntry {
        offset: 100,
        length: 50,
    };

    let versioned = VersionedSlotEntry {
        offset: 100,
        length: 50,
        xmin: test_txn(1),
        xmax: None,
        prev_version: None,
    };

    let regular_size = postcard::to_allocvec(&regular).unwrap().len();
    let versioned_size = postcard::to_allocvec(&versioned).unwrap().len();

    // Versioned should be larger (has xmin, xmax, prev_version)
    assert!(versioned_size > regular_size);
}
