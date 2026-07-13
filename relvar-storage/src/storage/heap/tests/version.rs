#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use tempfile::NamedTempFile;

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
        page_id: PageId(5),
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
                page_id: PageId(0),
                slot: 0,
            }),
        },
        VersionedSlotEntry {
            offset: 3000,
            length: 300,
            xmin: test_txn(4),
            xmax: Some(test_txn(5)),
            prev_version: Some(TupleId {
                page_id: PageId(1),
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

// Phase 2.2: Insert with Version tests

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
    let page = Page::from_data(PageId(0), page_data).unwrap();

    let slots = [versioned_page.slots[0].clone().unwrap()];
    let extracted = heap
        .extract_tuples_from_versioned_slots(&page, slots.iter())
        .unwrap();

    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted[0], tuple);
}

#[test]
fn test_sentry_extract_tuples_from_versioned_slots_corrupted_length() {
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
            length: (PAGE_SIZE + 10) as u32, // Intentional overflow past page size
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
    let page = Page::from_data(PageId(0), page_data).unwrap();

    let mut slots = [versioned_page.slots[0].clone().unwrap()];
    // Now intentionally corrupt the page entry for extract
    slots[0].length = (PAGE_SIZE + 10) as u32;

    let extracted_err = heap
        .extract_tuples_from_versioned_slots(&page, slots.iter())
        .unwrap_err();

    assert!(matches!(extracted_err, HeapError::Serialization(_)));
    if let HeapError::Serialization(msg) = extracted_err {
        assert!(msg.contains("Corrupted slot on page"));
    }
}

#[test]
fn test_sentry_validate_slot_bounds_offset_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let page = Page::new(PageId(0));

    // Test the extract_tuple_from_page missing bound limit branch, where offset is fine but we exceed page data bounds
    // to trigger "Corrupted slot on page ... points outside page data"
    let err = heap
        .extract_raw_tuple_data(&page, 0, (PAGE_SIZE + 10) as u32)
        .unwrap_err();

    assert!(matches!(err, HeapError::Serialization(_)));
    if let HeapError::Serialization(msg) = err {
        assert!(msg.contains("Corrupted slot on page"));
    }
}
