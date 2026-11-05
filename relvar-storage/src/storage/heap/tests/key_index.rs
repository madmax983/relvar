//! Heap-level maintenance of the implicit-key index (TTM RM Proscription 3).
//!
//! The engine-level behaviour is tested in `persistent_engine::tests::
//! implicit_key`; these tests drive the heap paths the engine reaches only
//! at open (recovery undo) or never (non-versioned writes, updates).
#![allow(unused_imports)]
use super::common::*;
use crate::storage::heap::key_index::KeyIndexState;
use crate::storage::heap::*;
use relvar_core::tuple;
use std::collections::HashSet;
use tempfile::NamedTempFile;

fn heap() -> (NamedTempFile, HeapFile) {
    let file = NamedTempFile::new().unwrap();
    let heap = HeapFile::create(file.path(), create_test_relation_type()).unwrap();
    (file, heap)
}

#[test]
fn test_versions_of_builds_index_and_finds_every_version() {
    let mut vpool = test_version_pool();
    let (_file, mut heap) = heap();
    let alice = tuple! { id: 1i64, name: "Alice" };
    heap.insert_tuple_versioned(&alice, test_txn(1), &mut vpool)
        .unwrap();
    assert!(!heap.key_index_is_built());

    assert_eq!(heap.versions_of(&alice, &mut vpool).unwrap().len(), 1);
    assert!(heap.key_index_is_built());

    // A second version (e.g. a re-insert after a delete) is indexed too.
    heap.insert_tuple_versioned(&alice, test_txn(2), &mut vpool)
        .unwrap();
    let mut creators: Vec<u64> = heap
        .versions_of(&alice, &mut vpool)
        .unwrap()
        .iter()
        .map(|v| v.xmin.value())
        .collect();
    creators.sort_unstable();
    assert_eq!(creators, vec![1, 2]);
    let bob = tuple! { id: 2i64, name: "Bob" };
    assert!(heap.versions_of(&bob, &mut vpool).unwrap().is_empty());
    heap.check_key_index(&mut vpool).unwrap();
}

#[test]
fn test_recovery_undo_removes_uncommitted_versions_from_index() {
    let mut vpool = test_version_pool();
    let (_file, mut heap) = heap();
    let alice = tuple! { id: 1i64, name: "Alice" };
    let bob = tuple! { id: 2i64, name: "Bob" };
    heap.insert_tuple_versioned(&alice, test_txn(1), &mut vpool)
        .unwrap();
    heap.versions_of(&alice, &mut vpool).unwrap(); // build
    heap.insert_tuple_versioned(&bob, test_txn(2), &mut vpool)
        .unwrap();
    heap.check_key_index(&mut vpool).unwrap();

    let committed: HashSet<_> = [test_txn(1)].into_iter().collect();
    assert_eq!(
        heap.undo_uncommitted_versions(&committed, &mut vpool)
            .unwrap(),
        1
    );

    assert!(heap.key_index_is_built());
    heap.check_key_index(&mut vpool).unwrap();
    assert!(heap.versions_of(&bob, &mut vpool).unwrap().is_empty());
}

#[test]
fn test_update_indexes_the_new_version() {
    let mut vpool = test_version_pool();
    let (_file, mut heap) = heap();
    let alice = tuple! { id: 1i64, name: "Alice" };
    let old = heap
        .insert_tuple_versioned(&alice, test_txn(1), &mut vpool)
        .unwrap();
    heap.versions_of(&alice, &mut vpool).unwrap(); // build
    let renamed = tuple! { id: 1i64, name: "Alicia" };
    heap.update_tuple_versioned(old, &renamed, test_txn(2), &mut vpool)
        .unwrap();
    heap.check_key_index(&mut vpool).unwrap();
    assert_eq!(heap.versions_of(&renamed, &mut vpool).unwrap().len(), 1);
}

#[test]
fn test_non_versioned_write_invalidates_index() {
    let mut vpool = test_version_pool();
    let (_file, mut heap) = heap();
    let alice = tuple! { id: 1i64, name: "Alice" };
    heap.versions_of(&alice, &mut vpool).unwrap(); // build (empty)
    assert!(heap.key_index_is_built());
    heap.insert_tuple(&alice).unwrap();
    assert!(!heap.key_index_is_built());
}

#[test]
fn test_consistency_check_detects_a_missing_entry() {
    let mut vpool = test_version_pool();
    let (_file, mut heap) = heap();
    let alice = tuple! { id: 1i64, name: "Alice" };
    let tuple_id = heap
        .insert_tuple_versioned(&alice, test_txn(1), &mut vpool)
        .unwrap();
    heap.versions_of(&alice, &mut vpool).unwrap(); // build
    let KeyIndexState::Built(index) = &mut heap.key_index else {
        panic!("index should be built");
    };
    let digest = index.digest(&alice);
    assert!(index.remove(digest, tuple_id));

    let error = heap.check_key_index(&mut vpool).unwrap_err();
    assert!(error.contains("missing from the index"), "{error}");
}
