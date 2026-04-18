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
fn test_heap_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    let rel_type = create_test_relation_type();

    // Write tuples
    {
        let mut heap = HeapFile::create(&path, rel_type.clone()).unwrap();
        heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
            .unwrap();
    }

    // Read them back in a new instance
    {
        let mut heap = HeapFile::open(&path, rel_type.clone()).unwrap();
        let results = heap.scan().unwrap();
        assert_eq!(results.len(), 2);
    }
}

#[test]
fn test_store_relation_returns_unit() {
    let temp_file = NamedTempFile::new().unwrap();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(temp_file.path(), rel_type.clone()).unwrap();

    let mut relation = Relation::new(rel_type);
    relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();

    // Type check: Verifies the API returns unit type, not Vec<TupleId>
    let result = heap.store_relation(&relation);
    assert!(result.is_ok());
    let _unit: () = result.unwrap();
}
