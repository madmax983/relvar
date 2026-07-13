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
fn test_sentry_validate_slot_bounds_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0); // Assuming PAGE_SIZE defaults or similar, but Page::new(0) creates empty page

    // Test out of bounds logic
    let result = heap.validate_slot_bounds(&page, 4000, 200); // 4000 + 200 = 4200 > 0
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_extract_raw_tuple_data_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0);

    // Test out of bounds logic
    let result = heap.extract_raw_tuple_data(&page, 4000, 200);
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_extract_tuples_from_versioned_slots_corruption() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0);

    // Create some corrupted versioned slots pointing out of bounds
    let slots = [crate::storage::heap::VersionedSlotEntry {
        offset: 4000,
        length: 200,
        xmin: crate::wal::TransactionId::new(0),
        xmax: None,
        prev_version: None,
    }];

    let result = heap.extract_tuples_from_versioned_slots(&page, slots.iter());

    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_extract_all_tuples_corruption() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0);

    // Create some corrupted normal slots pointing out of bounds
    let slots = vec![Some(crate::storage::heap::SlotEntry {
        offset: 4000,
        length: 200,
    })];

    let result = heap.extract_all_tuples(&page, &slots);

    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}

#[test]
fn test_sentry_validate_slot_bounds_overflow_addition() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0);

    // Test the offset addition overflow explicitly
    let result = heap.validate_slot_bounds(&page, u32::MAX, u32::MAX);

    // In a 64-bit platform, u32::MAX + u32::MAX does NOT overflow a usize.
    // It's about ~8 billion which is well under usize::MAX.
    // However, it will easily trigger the "> page.data().len()" error,
    // which proves we properly mapped the fallback check if not on a 32-bit arch.
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data") || msg.contains("overflow")
    ));
}

#[test]
fn test_sentry_deserialize_slotted_page_corruption() {
    let temp_file = NamedTempFile::new().unwrap();
    let _heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0);
    // Page is empty (0 bytes). Deserialization of a SlottedPage requires reading
    // headers and lengths, so it should fail on bounds.
    let result = crate::storage::heap::deserialize_bounded::<SlottedPage>(page.data());
    assert!(result.is_err());
}

#[test]
fn test_sentry_extract_tuple_from_page_corruption() {
    let temp_file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();
    let page = Page::new(0);

    // Using corrupted bounds to trigger inner validation error
    let result = heap.extract_tuple_from_page(&page, 4000, 200);
    assert!(matches!(
        result,
        Err(HeapError::Serialization(msg)) if msg.contains("points outside page data")
    ));
}
