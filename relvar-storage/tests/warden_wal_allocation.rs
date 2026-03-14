// Warden Integration Test for WAL Allocation Bomb DoS
// This validates that our mitigation prevents unbounded allocation.
// Since WalManager is internal, we trigger it through PersistentEngine.

use relvar_storage::PersistentEngine;
use std::fs::OpenOptions;
use tempfile::TempDir;

#[test]
fn test_wal_allocation_bomb_prevention_explicit_error() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path();
    let wal_path = db_path.join("wal.log");

    // Initialize an empty database
    let engine = PersistentEngine::open(db_path).unwrap();
    drop(engine); // Closes DB

    // The WAL file (wal.log) should now exist. We maliciously increase its size using sparse files.
    // 2GB + 1024 bytes
    let max_wal_size: u64 = 2 * 1024 * 1024 * 1024;
    let file = OpenOptions::new().write(true).open(&wal_path).unwrap();

    // Set file length without allocating real disk space (sparse file)
    file.set_len(max_wal_size + 1024).unwrap();
    drop(file);

    // Re-opening the engine triggers recovery, which should hit the `take(MAX_WAL_SIZE)` limit
    // and fail explicitly with an Io error for FileTooLarge, rather than an OOM crash.
    let result = PersistentEngine::open(db_path);

    match result {
        Err(e) => {
            let msg = e.to_string();
            // Assert we got the specific error message added in our defense
            assert!(
                msg.contains("WAL file size"),
                "Expected WAL size error, got: {}",
                msg
            );
            assert!(
                msg.contains("exceeds safety limit"),
                "Expected safety limit text in error"
            );
            assert!(msg.contains("OOM DoS"), "Expected OOM DoS mention in error");
        }
        Ok(_) => {
            panic!("Engine unexpectedly opened successfully despite massive corrupted WAL file!")
        }
    }
}
