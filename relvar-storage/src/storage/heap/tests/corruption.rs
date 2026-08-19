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
        Err(HeapError::Serialization(msg)) if msg.contains("Tuple end offset overflow") || msg.contains("overflow") || msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_validate_slot_bounds_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let data = vec![0u8; 100];
    let page = Page::from_data(0, data).unwrap();

    let result = heap.validate_slot_bounds(&page, u32::MAX, 10);
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("Tuple end offset overflow") || msg.contains("overflow") || msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_validate_slot_bounds_out_of_bounds() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let data = vec![0u8; 100];
    let page = Page::from_data(0, data).unwrap();

    let result = heap.validate_slot_bounds(&page, 95, 10);
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}
