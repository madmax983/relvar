#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use tempfile::NamedTempFile;

#[test]
fn test_check_tuple_size_limit_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let overflow_size = usize::MAX;
    let result = heap.check_tuple_size_limit(overflow_size);

    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("Header size + tuple data length overflow")
    ));
}

#[test]
fn test_check_versioned_tuple_size_limit_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let overflow_size = usize::MAX;
    let result = heap.check_versioned_tuple_size_limit(overflow_size, false);

    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("Header size + tuple data length overflow") || msg.contains("Format header + header size overflow")
    ));
}

#[test]
fn test_encode_versioned_page_rejects_oversized_slot_dir() {
    // Regression: an oversized slot directory must be rejected instead of
    // overflowing the page buffer (previously `verify_versioned_page_size`).
    let page = VersionedSlottedPage {
        magic: VERSIONED_PAGE_MAGIC,
        slot_count: 200_000,
        slots: vec![None; 200_000],
    };

    let result = encode_versioned_page(&page, &[]);

    match result {
        Err(SlottedError::Serialization(msg)) => {
            assert!(
                msg.contains("too large for page"),
                "unexpected message: {msg}"
            )
        }
        other => panic!("expected Serialization error for oversized slot dir, got {other:?}"),
    }
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

#[test]
fn test_encode_slotted_page_rejects_out_of_bounds_slot() {
    let tuple_data = vec![1u8, 2, 3, 4];

    // Slot claims tuple data past the end of the page buffer: previously panicked
    // on data[offset..offset + length].
    let past_end = SlottedPage {
        slot_count: 1,
        slots: vec![Some(SlotEntry {
            offset: (PAGE_SIZE - 8) as u32,
            length: 16,
        })],
    };
    let result = encode_slotted_page(&past_end, std::slice::from_ref(&tuple_data));
    match result {
        Err(SlottedError::Serialization(msg)) => {
            assert!(msg.contains("outside buffer"), "unexpected message: {msg}")
        }
        other => panic!("expected Serialization error for out-of-bounds slot, got {other:?}"),
    }

    // Slot overlaps the slot-directory header: previously overwrote header bytes.
    let overlaps_header = SlottedPage {
        slot_count: 1,
        slots: vec![Some(SlotEntry {
            offset: 0,
            length: 4,
        })],
    };
    let result = encode_slotted_page(&overlaps_header, &[tuple_data]);
    match result {
        Err(SlottedError::Serialization(msg)) => {
            assert!(msg.contains("outside buffer"), "unexpected message: {msg}")
        }
        other => panic!("expected Serialization error for header-overlapping slot, got {other:?}"),
    }
}

#[test]
fn test_sentry_extract_tuples_from_versioned_slots_corrupted_offset() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let data = vec![0u8; 100];
    let page = Page::from_data(0, data).unwrap();

    let slot = VersionedSlotEntry {
        offset: u32::MAX - 5,
        length: 10,
        xmin: crate::wal::TransactionId::new(1),
        xmax: None,
        prev_version: None,
    };
    let slots = [slot];

    let result = heap.extract_tuples_from_versioned_slots(&page, slots.iter());
    assert!(matches!(
        result,
        Err(HeapError::Slotted(SlottedError::Serialization(msg))) if msg.contains("Tuple end offset overflow") || msg.contains("overflow") || msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_slot_bytes_overflow() {
    let data = vec![0u8; 100];

    let result = slot_bytes(&data, u32::MAX, 10);
    assert!(matches!(
        result,
        Err(SlottedError::Serialization(msg)) if msg.contains("Tuple end offset overflow") || msg.contains("overflow") || msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_slot_bytes_out_of_bounds() {
    let data = vec![0u8; 100];

    let result = slot_bytes(&data, 95, 10);
    assert!(matches!(
        result,
        Err(SlottedError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}
