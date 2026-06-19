#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use relvar_core::values::Tuple;
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
