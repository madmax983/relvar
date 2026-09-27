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
    HeapFile::<crate::FileBlockDevice>::repack_versioned_slots(
        &mut versioned_page.slots,
        &existing_tuples,
        PAGE_SIZE - 8,
    )
    .unwrap();
    let page_data = encode_versioned_page(&versioned_page, &existing_tuples).unwrap();
    let page = Page::from_data(0, page_data).unwrap();

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
    HeapFile::<crate::FileBlockDevice>::repack_versioned_slots(
        &mut versioned_page.slots,
        &existing_tuples,
        PAGE_SIZE - 8,
    )
    .unwrap();
    let page_data = encode_versioned_page(&versioned_page, &existing_tuples).unwrap();
    let page = Page::from_data(0, page_data).unwrap();

    let mut slots = [versioned_page.slots[0].clone().unwrap()];
    // Now intentionally corrupt the page entry for extract
    slots[0].length = (PAGE_SIZE + 10) as u32;

    let extracted_err = heap
        .extract_tuples_from_versioned_slots(&page, slots.iter())
        .unwrap_err();

    assert!(matches!(extracted_err, HeapError::Slotted(_)));
    if let HeapError::Slotted(SlottedError::Serialization(msg)) = extracted_err {
        assert!(msg.contains("Corrupted slot points outside page data"));
    }
}

#[test]
fn test_sentry_extract_raw_tuple_data_out_of_bounds() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let page = Page::new(0);

    // Offset is fine but the range exceeds the page data bounds, triggering
    // the storage core's "Corrupted slot points outside page data" error.
    let err = heap
        .extract_raw_tuple_data(&page, 0, (PAGE_SIZE + 10) as u32)
        .unwrap_err();

    assert!(matches!(err, HeapError::Slotted(_)));
    if let HeapError::Slotted(SlottedError::Serialization(msg)) = err {
        assert!(msg.contains("Corrupted slot points outside page data"));
    }
}

// ---------------------------------------------------------------------------
// v0.9 claim/rollback protocol: every pooled write claims its version
// record before mutating any page, and a failure after the claim hands
// the record back. These tests prove no claim leaks on the failure paths.
// ---------------------------------------------------------------------------

#[test]
fn test_insert_buffer_exhaustion_releases_claim() {
    // One page working buffer: it can be held across the insert below so
    // the claim succeeds but the buffer checkout fails.
    let mut vpool = crate::mvcc::VersionPool::with_capacities(4, 1, 4);
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    // Permanently check out the only buffer (`mem::forget` skips the
    // guard's Drop so the checkout survives while `vpool` is reused).
    let held = vpool.acquire_buffer().unwrap();
    std::mem::forget(held);

    let tuple = tuple! { id: 1i64, name: "Alice" };
    let err = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap_err();
    assert!(
        matches!(
            err,
            HeapError::Mvcc(crate::mvcc::MvccError::VersionBuffersExhausted { .. })
        ),
        "expected typed VersionBuffersExhausted, got {err:?}"
    );
    // The claim rolled back: nothing is leaked and the pool is reusable.
    assert_eq!(vpool.used_versions(), 0);
}

#[test]
fn test_update_missing_tuple_releases_claim() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let tuple = tuple! { id: 1i64, name: "Alice" };
    heap.insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();
    vpool.release_for_txn(test_txn(1));

    // Slot 99 does not exist on the otherwise healthy page 0: the update
    // claims first, then fails the lookup before mutating anything.
    let missing = TupleId {
        page_id: 0,
        slot: 99,
    };
    let replacement = tuple! { id: 1i64, name: "Nobody" };
    let err = heap
        .update_tuple_versioned(missing, &replacement, test_txn(2), &mut vpool)
        .unwrap_err();
    assert!(matches!(err, HeapError::TupleNotFound));
    assert_eq!(vpool.used_versions(), 0);

    // The pool still serves writes afterwards.
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(3), &mut vpool)
        .unwrap();
    assert_eq!(vpool.used_versions(), 1);
    vpool.release_for_txn(test_txn(3));
    assert_eq!(vpool.used_versions(), 0);
    let _ = tuple_id;
}

#[test]
fn test_delete_missing_tuple_releases_claim() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let missing = TupleId {
        page_id: 0,
        slot: 7,
    };
    let err = heap
        .delete_tuple_versioned(missing, test_txn(1), &mut vpool)
        .unwrap_err();
    assert!(matches!(err, HeapError::TupleNotFound));
    assert_eq!(vpool.used_versions(), 0);
}

#[test]
fn test_version_pool_exhaustion_is_typed_and_leaves_pool_usable() {
    // Exactly one version record: the second concurrent write cannot
    // happen, and the failure must be typed, not a panic.
    let mut vpool = crate::mvcc::VersionPool::with_capacities(1, 1, 4);
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let first = tuple! { id: 1i64, name: "Alice" };
    heap.insert_tuple_versioned(&first, test_txn(1), &mut vpool)
        .unwrap();
    assert_eq!(vpool.used_versions(), 1);

    let second = tuple! { id: 2i64, name: "Bob" };
    let err = heap
        .insert_tuple_versioned(&second, test_txn(2), &mut vpool)
        .unwrap_err();
    assert!(
        matches!(
            err,
            HeapError::Mvcc(crate::mvcc::MvccError::VersionPoolExhausted {
                used: 1,
                capacity: 1
            })
        ),
        "expected typed VersionPoolExhausted, got {err:?}"
    );
    // The failed claim left the pool exactly as it found it.
    assert_eq!(vpool.used_versions(), 1);

    // Ending the first transaction frees its record; the write succeeds.
    vpool.release_for_txn(test_txn(1));
    heap.insert_tuple_versioned(&second, test_txn(2), &mut vpool)
        .unwrap();
    assert_eq!(vpool.used_versions(), 1);
}

#[test]
fn test_write_path_does_not_grow_pool_backing_storage() {
    // A realistic mix of pooled writes across several transactions must
    // not grow any of the pool's backing vectors after construction:
    // the write path is allocation-free past setup.
    let mut vpool = test_version_pool();
    let before = vpool.backing_capacities();
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let mut ids = Vec::new();
    for i in 0..50 {
        let t = tuple! { id: i as i64, name: "Name" };
        ids.push(
            heap.insert_tuple_versioned(&t, test_txn(1), &mut vpool)
                .unwrap(),
        );
    }
    for id in ids.iter().copied() {
        let t = tuple! { id: 1000i64, name: "Updated" };
        heap.update_tuple_versioned(id, &t, test_txn(2), &mut vpool)
            .unwrap();
    }
    for id in ids.iter().copied().take(25) {
        heap.delete_tuple_versioned(id, test_txn(3), &mut vpool)
            .unwrap();
    }
    vpool.release_for_txn(test_txn(1));
    vpool.release_for_txn(test_txn(2));
    vpool.release_for_txn(test_txn(3));

    assert_eq!(vpool.backing_capacities(), before);
    assert_eq!(vpool.used_versions(), 0);
}
