use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;

#[test]
fn test_scan_visible_sees_only_visible() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1 inserts and commits
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
        .unwrap();

    // T2 starts after T1 committed
    let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 1);
}
#[test]
fn test_scan_visible_skips_uncommitted() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1 inserts but doesn't commit
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
        .unwrap();

    // T2 starts
    let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
    let committed = HashSet::new(); // T1 not committed

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    // T2 should not see T1's uncommitted tuple
    assert_eq!(visible.len(), 0);
}
#[test]
fn test_scan_visible_sees_own_changes() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let txn_id = test_txn(1);

    // T1 inserts (uncommitted)
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, txn_id)
        .unwrap();

    // T1's snapshot
    let snapshot = TransactionSnapshot::new(txn_id, test_lsn(100), vec![]);
    let committed = HashSet::new(); // T1 not committed yet

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    // T1 should see its own uncommitted tuple
    assert_eq!(visible.len(), 1);
}
#[test]
fn test_scan_visible_concurrent_uncommitted() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1 inserts
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
        .unwrap();

    // T2 starts while T1 is active
    let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(100), vec![test_txn(1)]);
    let mut committed = HashSet::new();
    committed.insert(test_txn(1)); // T1 committed after T2 started

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    // T2 should not see T1's tuple (was active when T2 started)
    assert_eq!(visible.len(), 0);
}
#[test]
fn test_scan_visible_empty_relation() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let snapshot = TransactionSnapshot::new(test_txn(1), test_lsn(100), vec![]);
    let committed = HashSet::new();

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 0);
}
#[test]
fn test_scan_visible_multiple_committed() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1, T2, T3 insert and commit
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
        .unwrap();
    heap.insert_tuple_versioned(&tuple! { id: 2i64, name: "Bob" }, test_txn(2))
        .unwrap();
    heap.insert_tuple_versioned(&tuple! { id: 3i64, name: "Charlie" }, test_txn(3))
        .unwrap();

    // T4 starts after all committed
    let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));
    committed.insert(test_txn(3));

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 3);
}
#[test]
fn test_scan_visible_mixed_committed_uncommitted() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1 inserts and commits
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
        .unwrap();

    // T2 inserts but doesn't commit
    heap.insert_tuple_versioned(&tuple! { id: 2i64, name: "Bob" }, test_txn(2))
        .unwrap();

    // T3 inserts and commits
    heap.insert_tuple_versioned(&tuple! { id: 3i64, name: "Charlie" }, test_txn(3))
        .unwrap();

    // T4 starts
    let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    // T2 not committed
    committed.insert(test_txn(3));

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    // Should see T1 and T3, but not T2
    assert_eq!(visible.len(), 2);
}
#[test]
fn test_scan_visible_across_pages() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert large tuples to span multiple pages
    let data = vec![0u8; 3000];

    heap.insert_tuple_versioned(&tuple! { data: data.clone() }, test_txn(1))
        .unwrap();
    heap.insert_tuple_versioned(&tuple! { data: data.clone() }, test_txn(2))
        .unwrap();
    heap.insert_tuple_versioned(&tuple! { data: data.clone() }, test_txn(3))
        .unwrap();

    let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));
    committed.insert(test_txn(3));

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    // Should see all 3 tuples across multiple pages
    assert_eq!(visible.len(), 3);
}
#[test]
fn test_scan_visible_no_committed_set() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuples
    heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
        .unwrap();

    // Empty committed set
    let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
    let committed = HashSet::new();

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    // Should not see any tuples (none committed)
    assert_eq!(visible.len(), 0);
}
#[test]
fn test_scan_visible_preserves_tuple_data() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let original_tuple = tuple! { id: 42i64, name: "TestData" };
    heap.insert_tuple_versioned(&original_tuple, test_txn(1))
        .unwrap();

    let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));

    let visible = heap.scan_visible(&snapshot, &committed).unwrap();

    assert_eq!(visible.len(), 1);
    // Tuple data should be preserved (exact content verified by serialization)
    assert_eq!(visible[0], original_tuple);
}
#[test]
fn test_scan_visible_integer_overflow() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let txn_id = test_txn(1);
    heap.insert_tuple_versioned(&relvar_core::tuple! { id: 1i64, name: "Alice" }, txn_id)
        .unwrap();

    // Corrupt the page
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp_file.path())
        .unwrap();
    use std::io::{Read, Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE];
    file.read_exact(&mut page_data).unwrap();

    // Find slot dir and corrupt
    let mut versioned_page: VersionedSlottedPage = postcard::from_bytes(&page_data[5..]).unwrap();
    if !versioned_page.slots.is_empty() {
        if let Some(slot) = versioned_page.slots[0].as_mut() {
            slot.offset = u32::MAX;
            slot.length = u32::MAX;
        }
    } else {
        versioned_page.slots.push(Some(VersionedSlotEntry {
            offset: u32::MAX,
            length: u32::MAX,
            xmin: txn_id,
            xmax: None,
            prev_version: None,
        }));
    }

    let new_dir = postcard::to_allocvec(&versioned_page).unwrap();
    page_data[5..5 + new_dir.len()].copy_from_slice(&new_dir);

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&page_data).unwrap();

    // Test
    let mut heap = HeapFile::open(temp_file.path(), create_test_relation_type()).unwrap();
    let snapshot =
        crate::mvcc::TransactionSnapshot::new(test_txn(2), crate::wal::Lsn::new(100), vec![]);
    let committed = std::collections::HashSet::from([test_txn(1)]);
    let result = heap.scan_visible(&snapshot, &committed);

    match result {
        Err(HeapError::Page(crate::storage::PageError::Serialization(msg)))
            if msg.contains("Page data length") => {}
        Err(HeapError::Serialization(msg))
            if msg.contains("Tuple end offset overflow") || msg.contains("Page data length") => {}
        other => panic!("Expected Tuple end offset overflow error, got: {:?}", other),
    }
}
#[test]
fn test_scan_visible_out_of_bounds() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let txn_id = test_txn(1);
    heap.insert_tuple_versioned(&relvar_core::tuple! { id: 1i64, name: "Alice" }, txn_id)
        .unwrap();

    // Corrupt the page
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp_file.path())
        .unwrap();
    use std::io::{Read, Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE];
    file.read_exact(&mut page_data).unwrap();

    // Find slot dir and corrupt
    let mut versioned_page: VersionedSlottedPage = postcard::from_bytes(&page_data[5..]).unwrap();
    if !versioned_page.slots.is_empty() {
        if let Some(slot) = versioned_page.slots[0].as_mut() {
            // Set offset + length to > PAGE_SIZE but < u32::MAX to hit out-of-bounds branch
            slot.offset = PAGE_SIZE as u32 - 10;
            slot.length = 100;
        }
    } else {
        versioned_page.slots.push(Some(VersionedSlotEntry {
            offset: PAGE_SIZE as u32 - 10,
            length: 100,
            xmin: txn_id,
            xmax: None,
            prev_version: None,
        }));
    }

    let new_dir = postcard::to_allocvec(&versioned_page).unwrap();
    page_data[5..5 + new_dir.len()].copy_from_slice(&new_dir);

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&page_data).unwrap();

    // Test
    let mut heap = HeapFile::open(temp_file.path(), create_test_relation_type()).unwrap();
    let snapshot =
        crate::mvcc::TransactionSnapshot::new(test_txn(2), crate::wal::Lsn::new(100), vec![]);
    let committed = std::collections::HashSet::from([test_txn(1)]);
    let result = heap.scan_visible(&snapshot, &committed);

    match result {
        Err(HeapError::Page(crate::storage::PageError::Serialization(msg)))
            if msg.contains("Page data length") => {}
        Err(HeapError::Serialization(msg))
            if msg.contains("Corrupted slot points outside page data")
                || msg.contains("Page data length") => {}
        other => panic!("Expected Corrupted slot error, got: {:?}", other),
    }
}
