#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;

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
fn test_store_and_load_relation() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

    // Create a relation with some tuples
    let mut relation = Relation::new(rel_type.clone());
    relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    relation.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    relation
        .insert(tuple! { id: 3i64, name: "Charlie" })
        .unwrap();

    // Store it
    heap.store_relation(&relation).unwrap();

    // Load it back
    let loaded_relation = heap.load_relation().unwrap();

    assert_eq!(loaded_relation.cardinality(), 3);
    assert_eq!(loaded_relation, relation);
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

// Phase 5.1: Update Creates Version (TDD - RED)

#[test]
fn test_delete_already_deleted_sets_xmax_again() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

    // Delete by T2
    heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

    // Delete again by T3 (should succeed and update xmax)
    heap.delete_tuple_versioned(tuple_id, test_txn(3)).unwrap();

    // Verify xmax is now T3
    let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(3)));
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

// --- GC Tests moved from mvcc/gc.rs ---

// Phase 6.1: Dead Version Identification (TDD - RED)
