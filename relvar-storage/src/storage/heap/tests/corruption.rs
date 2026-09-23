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
fn test_serialize_slotted_page_rejects_out_of_bounds_slot() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
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
    let result =
        heap.serialize_slotted_page_with_tuples(&past_end, std::slice::from_ref(&tuple_data));
    match result {
        Err(HeapError::Serialization(msg)) => {
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
    let result = heap.serialize_slotted_page_with_tuples(&overlaps_header, &[tuple_data]);
    match result {
        Err(HeapError::Serialization(msg)) => {
            assert!(msg.contains("outside buffer"), "unexpected message: {msg}")
        }
        other => panic!("expected Serialization error for header-overlapping slot, got {other:?}"),
    }
}
