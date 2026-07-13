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

        let page = Page::from_data(PageId(i), page_data).unwrap();
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

    let page = Page::from_data(PageId(0), page_data).unwrap();
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

    let page = Page::from_data(PageId(0), page_data).unwrap();
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

#[cfg(test)]
mod offset_overflow_tests {
    use crate::storage::heap::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use tempfile::NamedTempFile;
    fn create_test_relation_type() -> RelationType {
        let heading = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        RelationType::new(heading)
    }
    #[allow(dead_code)]
    fn test_txn(value: u64) -> crate::wal::TransactionId {
        crate::wal::TransactionId::new(value)
    }
    #[test]
    fn test_heap_deserialize_short_page() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let heap = HeapFile::create(path, rel_type).unwrap();

        // Create a page that mimics versioned format but is too short
        // PAGE_FORMAT_VERSION (1 byte) + 4 bytes length = 5 bytes needed
        // We write 3 bytes: [PAGE_FORMAT_VERSION, 0, 0]
        let data = vec![PAGE_FORMAT_VERSION, 0, 0];
        let page = Page::from_data(PageId(0), data).unwrap();

        // Call private method directly to verify protection
        // (update_tuple_versioned calls this without is_versioned_page check)
        let result = heap.deserialize_versioned_page(&page);

        assert!(result.is_err());
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert_eq!(msg, "Versioned page too short to contain header");
            }
            _ => panic!("Expected specific Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_header_corruption() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create a relation type
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // We want to insert many small tuples to increase N (number of slots).
        // Discrepancy D = N bytes.
        // We want to fill the page such that we are just on the edge.

        for i in 0..1000 {
            let t = tuple! { id: i as i64 };
            heap.insert_tuple(&t).unwrap();

            // Read page 0
            let page = heap.page_file.read_page(PageId(0)).unwrap();
            // Try to deserialize
            let res = heap.deserialize_slotted_page(&page);
            if res.is_err() {
                println!("Corruption detected at insert {}!", i);
                panic!("Corruption detected: {:?}", res.err());
            }

            // Also verify that the last inserted tuple is valid
            let sp = res.unwrap();
            if let Some(Some(last_slot)) = sp.slots.last() {
                // Check for overlap
                let slot_dir = postcard::to_allocvec(&sp).unwrap();
                // Slot dir is at offset 0.
                // Tuple is at last_slot.offset.
                // If tuple start < slot_dir end, we have overlap.
                if last_slot.offset < slot_dir.len() as u32 {
                    println!(
                        "Overlap detected at insert {}! Offset: {}, Header: {}",
                        i,
                        last_slot.offset,
                        slot_dir.len()
                    );
                    panic!("Overlap detected!");
                }
            }

            if page.id() > PageId(0) {
                println!("Page split happened at insert {}", i);
                break;
            }
        }
    }

    #[test]
    fn test_allocation_bomb_prevention() {
        // Construct a malicious payload: a Vec<u8> with length prefix 1GB
        // Postcard uses varints. 1 GB is 2^30.
        let huge_len: usize = 1024 * 1024 * 1024; // 1 GB
        let mut payload = Vec::new();
        // Since postcard uses varint, we must serialize the length the way postcard does
        payload.extend_from_slice(&postcard::to_allocvec(&huge_len).unwrap());
        // No actual data follows

        // Try to deserialize into Vec<u8> using our bounded deserializer
        // This should fail immediately because postcard checks if enough bytes are remaining
        let result: Result<Vec<u8>, HeapError> = deserialize_bounded(&payload);

        assert!(result.is_err());
        match result {
            Err(HeapError::Serialization(_)) => {
                // Expected error
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_slot_reuse_corruption() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Insert 3 tuples: A, B, C
        let tuple_a = tuple! { id: 1i64, name: "A" };
        let tuple_b = tuple! { id: 2i64, name: "B" };
        let tuple_c = tuple! { id: 3i64, name: "C" };

        heap.insert_tuple(&tuple_a).unwrap();
        heap.insert_tuple(&tuple_b).unwrap();
        heap.insert_tuple(&tuple_c).unwrap();

        // Verify initial state
        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 3);

        // 2. Manually simulate deletion of B (slot 1) to force reuse
        // We do this by modifying the page directly since we don't have a public delete yet
        {
            let page = heap.page_file.read_page(PageId(0)).unwrap();
            let mut sp = heap.deserialize_slotted_page(&page).unwrap();

            // Delete slot 1 (B)
            sp.slots[1] = None;

            // Extract existing tuples (A, B, C)
            // Note: extract_all_tuples returns Vec<Vec<u8>> corresponding to slots
            // But we just modified slots[1] to None!
            // So we need to be careful.
            // Let's re-read the page as it was on disk to get the data
            let original_sp = heap.deserialize_slotted_page(&page).unwrap();
            let mut existing_tuples = heap.extract_all_tuples(&page, &original_sp.slots).unwrap();

            // Mark tuple B as empty
            existing_tuples[1] = Vec::new();

            // Repack and write back
            HeapFile::repack_slots(&mut sp.slots, &existing_tuples, USABLE_PAGE_SIZE_V1).unwrap();
            let new_page_data = heap
                .serialize_slotted_page_with_tuples(&sp, &existing_tuples)
                .unwrap();
            let new_page = Page::from_data(PageId(0), new_page_data).unwrap();
            heap.page_file.write_page(&new_page).unwrap();
        }

        // Verify B is gone
        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 2);
        assert!(tuples.contains(&tuple_a));
        assert!(tuples.contains(&tuple_c));

        // 3. Insert tuple D. Should reuse slot 1.
        let tuple_d = tuple! { id: 4i64, name: "D" };
        heap.insert_tuple(&tuple_d).unwrap();

        // 4. Verify all tuples are present and correct
        let tuples = heap.scan().unwrap();

        // If corruption happened, C might be lost or corrupted
        assert_eq!(tuples.len(), 3, "Expected 3 tuples (A, C, D)");

        assert!(tuples.contains(&tuple_a), "Missing tuple A");
        assert!(tuples.contains(&tuple_d), "Missing tuple D");
        assert!(
            tuples.contains(&tuple_c),
            "Missing tuple C - CORRUPTION DETECTED!"
        );
    }

    #[test]
    fn test_repack_slots_correctness() {
        // Test edge case where tuples perfectly fill the page
        let mut slots: Vec<Option<SlotEntry>> = vec![];
        let mut tuples: Vec<Vec<u8>> = vec![];

        // Add 2 tuples, each 100 bytes
        slots.push(Some(SlotEntry {
            offset: 0,
            length: 100,
        }));
        tuples.push(vec![0u8; 100]);

        slots.push(Some(SlotEntry {
            offset: 0,
            length: 100,
        }));
        tuples.push(vec![0u8; 100]);

        // Usable size = 200
        let usable_size = 200;

        HeapFile::repack_slots(&mut slots, &tuples, usable_size).unwrap();

        // Check offsets
        // Last tuple (index 1) gets offset: 200 - 100 = 100
        assert_eq!(slots[1].as_ref().unwrap().offset, 100);
        assert_eq!(slots[1].as_ref().unwrap().length, 100);

        // First tuple (index 0) gets offset: 100 - 100 = 0
        assert_eq!(slots[0].as_ref().unwrap().offset, 0);
        assert_eq!(slots[0].as_ref().unwrap().length, 100);

        // Test overflow (too many tuples)
        let tuples_overflow = vec![vec![0u8; 100], vec![0u8; 100], vec![0u8; 1]]; // Total 201
        let mut slots_overflow = vec![
            Some(SlotEntry {
                offset: 0,
                length: 100,
            }),
            Some(SlotEntry {
                offset: 0,
                length: 100,
            }),
            Some(SlotEntry {
                offset: 0,
                length: 1,
            }),
        ];

        let result = HeapFile::repack_slots(&mut slots_overflow, &tuples_overflow, usable_size);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HeapError::PageFull));
    }
}

#[cfg(test)]
mod security_tests {
    use crate::storage::heap::*;
    use relvar_core::tuple;
    use relvar_core::types::{ScalarType, TupleType};
    use tempfile::NamedTempFile;

    #[test]
    fn test_update_versioned_tuple_too_large_prevents_loop() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Find the critical payload size dynamically
        let mut critical_payload = None;

        // Loop through sizes that are close to page limit
        // USABLE_PAGE_SIZE is 4088. Overhead roughly 40-60 bytes.
        // We look for a size where it fits with 'None' but fails with 'Some'
        for len in 3500..4088 {
            let data = vec![0u8; len];
            let tuple = tuple! { data: data.clone() };
            // Serialize to get length
            let tuple_data = postcard::to_allocvec(&tuple).unwrap();
            let tuple_data_len = tuple_data.len();

            // Check if it fits with None (insert)
            let fits_insert = heap
                .check_versioned_tuple_size_limit(tuple_data_len, false)
                .is_ok();

            // Check if it fits with Some (update)
            let fits_update = heap
                .check_versioned_tuple_size_limit(tuple_data_len, true)
                .is_ok();

            if fits_insert && !fits_update {
                println!("Found critical payload length: {}", len);
                critical_payload = Some(len);
                break;
            }
        }

        let len = critical_payload.expect("Failed to find critical payload size");
        let data = vec![0u8; len];
        let tuple = tuple! { data: data };

        // Insert should succeed
        let tid = heap
            .insert_tuple_versioned(&tuple, crate::wal::TransactionId::new(1))
            .expect("Insert failed");

        // Update should fail with TupleTooLarge, NOT loop forever
        // If the bug exists, this call would loop forever (or timeout)
        // With the fix, it should return TupleTooLarge
        let result = heap.update_tuple_versioned(tid, &tuple, crate::wal::TransactionId::new(2));

        assert!(result.is_err());
        match result {
            Err(HeapError::TupleTooLarge(_)) => {
                println!("Caught TupleTooLarge as expected");
            }
            Err(HeapError::PageFull) => {
                panic!("Got PageFull - vulnerability likely present");
            }
            _ => panic!("Expected TupleTooLarge, got {:?}", result),
        }
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
        page_id: PageId(0),
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
        page_id: PageId(0),
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
        page_id: PageId(0),
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
        page_id: PageId(0),
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

// --- GC Tests moved from mvcc/gc.rs ---

// Phase 6.1: Dead Version Identification (TDD - RED)
