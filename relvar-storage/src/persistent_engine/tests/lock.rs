//! Regression tests: a database directory is opened by at most one engine
//! at a time.
//!
//! Two engines on one directory would interleave WAL appends and heap
//! page writes (corrupting both), hand out the same transaction ids, and
//! keep separate in-memory key indexes (breaking key uniqueness).

use super::common::*;
use crate::persistent_engine::*;
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use tempfile::TempDir;

#[test]
fn test_second_open_of_locked_directory_fails() {
    let dir = TempDir::new().unwrap();
    let _first = PersistentEngine::open(dir.path()).unwrap();

    let second = PersistentEngine::open(dir.path());
    let error = match second {
        Ok(_) => panic!("a second engine must not open a locked directory"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("locked by another"),
        "the error must say the directory is in use: {error}"
    );
}

#[test]
fn test_failed_second_open_leaves_first_engine_working() {
    let dir = TempDir::new().unwrap();
    let mut first = PersistentEngine::open(dir.path()).unwrap();
    first.create_relation("TEST", test_rel_type()).unwrap();

    assert!(PersistentEngine::open(dir.path()).is_err());

    first
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    drop(first);
    let reopened = PersistentEngine::open(dir.path()).unwrap();
    assert_eq!(reopened.load_relation("TEST").unwrap().cardinality(), 1);
}

#[test]
fn test_directory_reopens_after_engine_drop() {
    let dir = TempDir::new().unwrap();
    for _ in 0..3 {
        let engine = PersistentEngine::open(dir.path()).unwrap();
        drop(engine);
    }
}

#[test]
fn test_lock_is_released_when_open_fails_after_locking() {
    // Recovery fails (corrupt WAL): the lock must not outlive the failed
    // open, or the directory could never be opened again after repair.
    let dir = TempDir::new().unwrap();
    drop(PersistentEngine::open(dir.path()).unwrap());
    let wal = dir.path().join("wal.log");
    let good = std::fs::read(&wal).unwrap();
    std::fs::write(&wal, b"NOTAWAL!").unwrap();
    assert!(PersistentEngine::open(dir.path()).is_err());

    std::fs::write(&wal, good).unwrap();
    PersistentEngine::open(dir.path()).unwrap();
}
