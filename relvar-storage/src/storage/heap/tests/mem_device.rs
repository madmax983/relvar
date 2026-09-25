//! Heap file over a non-file [`BlockDevice`]: the in-memory device from the
//! storage core. Proves the heap file is generic over the HAL, not tied to
//! [`FileBlockDevice`].

use super::common::*;
use crate::storage::heap::*;
use relvar_core::tuple;
use relvar_storage_core::device::MemBlockDevice;
use std::collections::HashSet;

#[test]
fn test_heap_on_mem_device_insert_scan_roundtrip() {
    let device = MemBlockDevice::new();
    let mut heap = HeapFile::create_on_device(device, create_test_relation_type());

    let tuples = vec![
        tuple! { id: 1i64, name: "Alice" },
        tuple! { id: 2i64, name: "Bob" },
        tuple! { id: 3i64, name: "Carol" },
    ];
    for tuple in &tuples {
        heap.insert_tuple(tuple).unwrap();
    }

    let scanned = heap.scan().unwrap();
    assert_eq!(scanned.len(), tuples.len());
    for tuple in &tuples {
        assert!(
            scanned.contains(tuple),
            "inserted tuple missing from scan: {tuple:?}"
        );
    }
}

#[test]
fn test_heap_on_mem_device_versioned_roundtrip() {
    // The MVCC path also works on a non-file device.
    let device = MemBlockDevice::new();
    let mut heap = HeapFile::create_on_device(device, create_test_relation_type());

    let txn_id = test_txn(1);
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, txn_id)
        .unwrap();

    let snapshot = crate::mvcc::TransactionSnapshot::new(txn_id, test_lsn(1), vec![]);
    let mut committed = HashSet::new();
    committed.insert(txn_id);
    let visible = heap.scan_visible(&snapshot, &committed).unwrap();
    assert_eq!(visible.len(), 1);
}
