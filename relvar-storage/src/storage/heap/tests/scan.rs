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
fn test_heap_scan() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

    heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
        .unwrap();
    heap.insert_tuple(&tuple! { id: 3i64, name: "Charlie" })
        .unwrap();

    let results = heap.scan().unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn test_heap_scan_returns_only_tuples() {
    let temp_file = NamedTempFile::new().unwrap();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

    heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    // Type check: Verifies the API returns Vec<Tuple>, not Vec<(TupleId, Tuple)>
    let tuples: Vec<Tuple> = heap.scan().unwrap();
    assert_eq!(tuples.len(), 2);
}

#[test]
fn test_heap_scan_large_volume() {
    const NUM_PAGES: u64 = 1005;
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create a relation type with a Bytes attribute
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("data", ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Manually craft pages to avoid O(N^2) insert performance
    // We just need > 1000 pages, the content size doesn't matter as long as it's valid
    let payload = vec![0u8; 100];

    for i in 0..NUM_PAGES {
        let tuple = tuple! {
            id: i as i64,
            data: payload.clone(),
        };
        let tuple_data = postcard::to_allocvec(&tuple).unwrap();

        // Construct a SlottedPage with one tuple
        // We place tuple at the end of the page (standard behavior)
        let tuple_len = tuple_data.len();
        // USABLE_PAGE_SIZE = PAGE_SIZE - 8
        let offset = (PAGE_SIZE - 8) - tuple_len;

        let slotted_page = SlottedPage {
            slot_count: 1,
            slots: vec![Some(SlotEntry {
                offset: offset as u32,
                length: tuple_len as u32,
            })],
        };

        // We can use the helper method since we are in the same module (tests)
        let page_data = heap
            .serialize_slotted_page_with_tuples(&slotted_page, &[tuple_data])
            .unwrap();

        let page = Page::from_data(i, page_data).unwrap();
        heap.page_file.write_page(&page).unwrap();
    }

    // Scan and verify count
    let tuples = heap.scan().unwrap();
    assert_eq!(
        tuples.len() as u64,
        NUM_PAGES,
        "Scan stopped early! Expected {} tuples, got {}",
        NUM_PAGES,
        tuples.len()
    );
}

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
fn test_heap_scan_corrupted_slot() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create a manually corrupted page
    // Slot points to offset > PAGE_SIZE
    let slotted_page = SlottedPage {
        slot_count: 1,
        slots: vec![Some(SlotEntry {
            offset: (PAGE_SIZE + 100) as u32, // Invalid offset
            length: 10,
        })],
    };

    // Serialize just the header (no tuple data needed as offset is invalid)
    let slot_dir = postcard::to_allocvec(&slotted_page).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE - 8];
    page_data[..slot_dir.len()].copy_from_slice(&slot_dir);

    let page = Page::from_data(0, page_data).unwrap();
    heap.page_file.write_page(&page).unwrap();

    // Scan should fail
    let result = heap.scan();
    assert!(result.is_err());
    match result {
        Err(HeapError::Serialization(msg)) => {
            assert!(msg.contains("Corrupted slot"));
        }
        _ => panic!("Expected Serialization error for corrupted slot"),
    }
}

#[test]
fn test_heap_scan_mid_stream_corruption() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path();

    // 1. Create a large enough tuple to fill most of a page
    let large_data = vec![0u8; 3000];
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("data", ScalarType::Bytes);
    let rel_type_large = RelationType::new(heading);

    let mut heap = HeapFile::create(path, rel_type_large)?;

    // 2. Insert 3 tuples, each should land on a separate page
    for i in 0..3 {
        let tuple = tuple! {
            id: i as i64,
            data: large_data.clone(),
        };
        heap.insert_tuple(&tuple)?;
    }

    // Verify initial scan works
    let results = heap.scan()?;
    assert_eq!(results.len(), 3);

    // 3. Corrupt the middle page (Page 1)
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = std::fs::OpenOptions::new().write(true).open(path)?;

        // Seek to start of Page 1
        file.seek(SeekFrom::Start(PAGE_SIZE as u64))?;

        // Skip page length prefix (8 bytes) to corrupt the actual content
        file.seek(SeekFrom::Current(8))?;

        // Write garbage
        file.write_all(&[0xFF; 100])?;
        file.sync_all()?;
    }

    // 4. Verify scan fails gracefully
    let result = heap.scan();
    assert!(result.is_err());
    // Should be a serialization error because postcard will fail to deserialize garbage
    match result {
        Err(HeapError::Serialization(_)) => {}
        _ => panic!("Expected Serialization error, got {:?}", result),
    }

    Ok(())
}

#[test]
fn test_heap_scan_offset_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Create a page with an overflowing offset
    // offset = u32::MAX - 10, length = 20
    // offset + length overflows u32
    let corrupted_slot = SlotEntry {
        offset: u32::MAX - 10,
        length: 20,
    };

    let slotted_page = SlottedPage {
        slot_count: 1,
        slots: vec![Some(corrupted_slot)],
    };

    // Serialize header only
    let slot_dir = postcard::to_allocvec(&slotted_page).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE - 8];
    page_data[..slot_dir.len()].copy_from_slice(&slot_dir);

    let page = Page::from_data(0, page_data).unwrap();
    heap.page_file.write_page(&page).unwrap();

    // Scan should fail cleanly
    let result = heap.scan();
    assert!(result.is_err());
    match result {
        Err(HeapError::Serialization(msg)) => {
            // On 64-bit systems, u32+u32 fits in usize, so we get bounds check error.
            // On 32-bit systems, we get overflow error.
            assert!(
                msg.contains("Tuple end offset overflow") || msg.contains("Corrupted slot"),
                "Unexpected error message: {}",
                msg
            );
        }
        _ => panic!(
            "Expected Serialization error with overflow message, got {:?}",
            result
        ),
    }
}

#[test]
fn test_read_tuple_integer_overflow() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    // Insert a normal tuple
    heap.insert_tuple(&relvar_core::tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Corrupt the page to test `get_tuple` overflow
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp_file.path())
        .unwrap();
    use std::io::{Read, Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE];
    file.read_exact(&mut page_data).unwrap();

    // Find and corrupt the slot directory
    let mut slotted_page: SlottedPage = postcard::from_bytes(&page_data[5..]).unwrap();

    // Corrupt the first slot
    if !slotted_page.slots.is_empty() {
        if let Some(slot) = slotted_page.slots[0].as_mut() {
            slot.offset = u32::MAX;
            slot.length = u32::MAX;
        }
    } else {
        slotted_page.slots.push(Some(SlotEntry {
            offset: u32::MAX,
            length: u32::MAX,
        }));
    }

    // Write it back
    let new_data = postcard::to_allocvec(&slotted_page).unwrap();
    page_data[5..5 + new_data.len()].copy_from_slice(&new_data);

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&page_data).unwrap();

    // Re-open and try to get tuple
    let mut heap = HeapFile::open(temp_file.path(), create_test_relation_type()).unwrap();
    let result = heap.read_tuple(TupleId {
        page_id: 0,
        slot: 0,
    });

    match result {
        Err(HeapError::Page(crate::storage::PageError::Serialization(msg)))
            if msg.contains("Page data length") => {}
        Err(HeapError::Serialization(msg))
            if msg.contains("Tuple end offset overflow") || msg.contains("Page data length") => {}
        other => panic!("Expected Tuple end offset overflow error, got: {:?}", other),
    }
}

#[test]
fn test_read_tuple_out_of_bounds() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    // Insert a normal tuple
    heap.insert_tuple(&relvar_core::tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Corrupt the page to test `get_tuple` out of bounds
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp_file.path())
        .unwrap();
    use std::io::{Read, Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut page_data = vec![0u8; PAGE_SIZE];
    file.read_exact(&mut page_data).unwrap();

    // Find and corrupt the slot directory
    let mut slotted_page: SlottedPage = postcard::from_bytes(&page_data[5..]).unwrap();

    // Corrupt the first slot
    if !slotted_page.slots.is_empty() {
        if let Some(slot) = slotted_page.slots[0].as_mut() {
            slot.offset = PAGE_SIZE as u32 - 10;
            slot.length = 100;
        }
    } else {
        slotted_page.slots.push(Some(SlotEntry {
            offset: PAGE_SIZE as u32 - 10,
            length: 100,
        }));
    }

    // Write it back
    let new_data = postcard::to_allocvec(&slotted_page).unwrap();
    page_data[5..5 + new_data.len()].copy_from_slice(&new_data);

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&page_data).unwrap();

    // Re-open and try to get tuple
    let mut heap = HeapFile::open(temp_file.path(), create_test_relation_type()).unwrap();
    let result = heap.read_tuple(TupleId {
        page_id: 0,
        slot: 0,
    });

    match result {
        Err(HeapError::Page(crate::storage::PageError::Serialization(msg)))
            if msg.contains("Page data length") => {}
        Err(HeapError::Serialization(msg))
            if msg.contains("Corrupted slot points outside page data")
                || msg.contains("Page data length") => {}
        other => panic!("Expected Corrupted slot error, got: {:?}", other),
    }
}

#[test]
fn test_read_tuple_versioned_integer_overflow() {
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
    let result = heap.read_tuple_versioned(TupleId {
        page_id: 0,
        slot: 0,
    });

    match result {
        Err(HeapError::Page(crate::storage::PageError::Serialization(msg)))
            if msg.contains("Page data length") => {}
        Err(HeapError::Serialization(msg))
            if msg.contains("Tuple end offset overflow") || msg.contains("Page data length") => {}
        other => panic!("Expected Tuple end offset overflow error, got: {:?}", other),
    }
}

#[test]
fn test_read_tuple_versioned_out_of_bounds() {
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
    let result = heap.read_tuple_versioned(TupleId {
        page_id: 0,
        slot: 0,
    });

    match result {
        Err(HeapError::Page(crate::storage::PageError::Serialization(msg)))
            if msg.contains("Page data length") => {}
        Err(HeapError::Serialization(msg))
            if msg.contains("Corrupted slot points outside page data")
                || msg.contains("Page data length") => {}
        other => panic!("Expected Corrupted slot error, got: {:?}", other),
    }
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
