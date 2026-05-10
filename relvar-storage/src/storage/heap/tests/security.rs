use super::common::*;
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
