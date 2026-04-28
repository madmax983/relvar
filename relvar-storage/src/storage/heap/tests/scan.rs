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
