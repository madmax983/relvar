#![allow(unused_imports)]
use crate::storage::heap::*;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;
use crate::wal::{Lsn, TransactionId};
use crate::mvcc::TransactionSnapshot;
use super::common::*;

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
        let page = Page::from_data(0, data).unwrap();

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
            let page = heap.page_file.read_page(0).unwrap();
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

            if page.id() > 0 {
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
            let page = heap.page_file.read_page(0).unwrap();
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
            let new_page = Page::from_data(0, new_page_data).unwrap();
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
