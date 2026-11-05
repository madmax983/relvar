//! Regression tests: a torn final WAL frame (crash mid-append) ends the
//! log instead of making the database unopenable, while corruption in the
//! middle of the log is still rejected.

use super::common::*;
use crate::persistent_engine::*;
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use relvar_core::values::ScalarValue;
use std::path::Path;
use tempfile::TempDir;

/// Byte ranges `[start, end)` of every v2 frame in a WAL file, parsed from
/// the frame headers (`[lsn u64][len u64][payload][crc u32]` after the
/// 8-byte magic).
fn frame_ranges(wal: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut offset = 8;
    while offset < wal.len() {
        let len = u64::from_le_bytes(wal[offset + 8..offset + 16].try_into().unwrap()) as usize;
        let end = offset + 16 + len + 4;
        ranges.push((offset, end));
        offset = end;
    }
    ranges
}

/// Copies every file of `from` into `to`.
fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

fn names(engine: &PersistentEngine) -> Vec<String> {
    let relation = engine.load_relation("TEST").unwrap();
    let mut names: Vec<String> = relation
        .tuples()
        .filter_map(|t| match t.get("name") {
            Some(ScalarValue::String(name)) => Some(name.clone()),
            _ => None,
        })
        .collect();
    names.sort();
    names
}

/// A database whose WAL ends with Bob's transaction's Commit frame.
fn committed_alice_and_bob() -> TempDir {
    let dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();
    drop(engine);
    dir
}

#[test]
fn test_open_tolerates_torn_final_frame_at_every_offset() {
    let source = committed_alice_and_bob();
    let wal = std::fs::read(source.path().join("wal.log")).unwrap();
    let (last_start, last_end) = *frame_ranges(&wal).last().unwrap();
    assert_eq!(last_end, wal.len());

    for cut in last_start..last_end {
        let dir = TempDir::new().unwrap();
        copy_dir(source.path(), dir.path());
        std::fs::write(dir.path().join("wal.log"), &wal[..cut]).unwrap();

        let mut engine = PersistentEngine::open(dir.path())
            .unwrap_or_else(|e| panic!("open failed with the WAL cut at {cut}: {e}"));
        // Bob's Commit frame is torn: his transaction never committed.
        assert_eq!(names(&engine), vec!["Alice"], "cut at {cut}");

        // The torn bytes were truncated: appending and reopening works.
        engine
            .insert_tuple("TEST", tuple! { id: 3i64, name: "Carol" })
            .unwrap();
        drop(engine);
        let engine = PersistentEngine::open(dir.path())
            .unwrap_or_else(|e| panic!("reopen failed after cut at {cut}: {e}"));
        assert_eq!(names(&engine), vec!["Alice", "Carol"], "cut at {cut}");
    }
}

#[test]
fn test_open_tolerates_torn_wal_magic() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("wal.log"), &b"RELVAR02"[..3]).unwrap();

    let mut engine = PersistentEngine::open(dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    drop(engine);
    let engine = PersistentEngine::open(dir.path()).unwrap();
    assert_eq!(names(&engine), vec!["Alice"]);
}

#[test]
fn test_open_rejects_corruption_followed_by_valid_frames() {
    let dir = committed_alice_and_bob();
    let path = dir.path().join("wal.log");
    let mut wal = std::fs::read(&path).unwrap();
    let ranges = frame_ranges(&wal);
    assert!(ranges.len() > 3);
    // Flip a payload byte of the second frame: valid frames follow it.
    let (start, _) = ranges[1];
    wal[start + 16] ^= 0xFF;
    std::fs::write(&path, &wal).unwrap();

    let result = PersistentEngine::open(dir.path());
    assert!(
        result.is_err(),
        "mid-log corruption must be rejected, not truncated away"
    );
    // Nothing was truncated: the file is untouched.
    assert_eq!(std::fs::read(&path).unwrap(), wal);
}

#[test]
fn test_open_rejects_corrupted_length_followed_by_valid_frames() {
    let dir = committed_alice_and_bob();
    let path = dir.path().join("wal.log");
    let mut wal = std::fs::read(&path).unwrap();
    let ranges = frame_ranges(&wal);
    // Corrupt the first frame's length field so it claims to run past EOF.
    let (start, _) = ranges[0];
    let past_eof = (wal.len() as u64).to_le_bytes();
    wal[start + 8..start + 16].copy_from_slice(&past_eof);
    std::fs::write(&path, &wal).unwrap();

    assert!(PersistentEngine::open(dir.path()).is_err());
}
