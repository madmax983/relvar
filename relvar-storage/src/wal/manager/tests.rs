use super::*;
use super::*;
use crate::wal::TransactionId;
use tempfile::NamedTempFile;
#[test]
fn test_open_invalid_magic_header() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();

    // Write invalid magic header
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        file.write_all(b"INVALID_").unwrap();
    }

    let result = WalManager::open(&path);
    assert!(result.is_err());
    match result {
        Err(WalError::Corrupted(_, msg)) => {
            assert!(msg.contains("Invalid WAL magic header"));
        }
        _ => panic!("Expected Corrupted error"),
    }
}

#[test]
fn test_scan_corrupted_record_deserialization() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();

    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        use std::io::Write;

        // Write magic header
        file.write_all(WAL_MAGIC).unwrap();

        // Write a validly formatted record but with un-deserializable payload
        let lsn = 1u64;
        let bad_payload = vec![0xFF; 20]; // Invalid bincode payload for WalRecord
        let len = bad_payload.len() as u64;

        file.write_all(&lsn.to_le_bytes()).unwrap();
        file.write_all(&len.to_le_bytes()).unwrap();
        file.write_all(&bad_payload).unwrap();
    }

    let mut wal = WalManager::open(&path).unwrap();
    let result = wal.scan();

    assert!(result.is_err());
    match result {
        Err(WalError::Corrupted(_, msg)) => {
            assert!(msg.contains("Deserialization failed"));
        }
        _ => panic!("Expected Corrupted error from deserialization"),
    }
}

#[test]
fn test_scan_partial_record() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();

    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        use std::io::Write;

        // Write magic header
        file.write_all(WAL_MAGIC).unwrap();

        // Write partial LSN
        file.write_all(&[1, 2, 3]).unwrap();
    }

    let mut wal = WalManager::open(&path).unwrap();
    // Scan handles partial LSN gracefully by breaking the loop, returning Ok with empty records.
    let result = wal.scan();
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_create_wal() {
    let temp = NamedTempFile::new().unwrap();
    let wal = WalManager::create(temp.path()).unwrap();

    assert_eq!(wal.current_lsn(), Lsn::new(1));
    assert_eq!(wal.flush_lsn(), Lsn::new(0));
    assert!(wal.is_buffer_empty());
}

#[test]
fn test_log_appends_to_buffer() {
    let temp = NamedTempFile::new().unwrap();
    let mut wal = WalManager::create(temp.path()).unwrap();

    let txn_id = TransactionId::new(1);
    let record = WalRecord::Begin { txn_id };

    assert!(wal.is_buffer_empty());

    let lsn = wal.log(record).unwrap();

    assert_eq!(lsn, Lsn::new(1));
    assert!(!wal.is_buffer_empty());
    assert_eq!(wal.current_lsn(), Lsn::new(2));
}

#[test]
fn test_flush_writes_to_file() {
    let temp = NamedTempFile::new().unwrap();
    let mut wal = WalManager::create(temp.path()).unwrap();

    let txn_id = TransactionId::new(1);
    wal.log(WalRecord::Begin { txn_id }).unwrap();
    wal.log(WalRecord::Commit { txn_id }).unwrap();

    assert!(!wal.is_buffer_empty());

    wal.flush().unwrap();

    assert!(wal.is_buffer_empty());
    assert_eq!(wal.flush_lsn(), Lsn::new(2));
}

#[test]
fn test_flush_clears_buffer() {
    let temp = NamedTempFile::new().unwrap();
    let mut wal = WalManager::create(temp.path()).unwrap();

    let txn_id = TransactionId::new(1);
    wal.log(WalRecord::Begin { txn_id }).unwrap();

    let size_before = wal.buffer_size();
    assert!(size_before > 0);

    wal.flush().unwrap();

    assert_eq!(wal.buffer_size(), 0);
}

#[test]
fn test_auto_flush_on_capacity() {
    let temp = NamedTempFile::new().unwrap();
    let mut wal = WalManager::create(temp.path()).unwrap();

    // Override buffer capacity to small size for testing
    wal.buffer_capacity = 128;

    let txn_id = TransactionId::new(1);

    // Log records until auto-flush triggers
    for _ in 0..10 {
        wal.log(WalRecord::Begin { txn_id }).unwrap();
    }

    // Buffer should have been flushed at least once
    // (flush_lsn > 0 indicates at least one flush occurred)
    assert!(wal.flush_lsn().value() > 0);
}

#[test]
fn test_lsn_monotonic_after_flush() {
    let temp = NamedTempFile::new().unwrap();
    let mut wal = WalManager::create(temp.path()).unwrap();

    let txn_id = TransactionId::new(1);

    let lsn1 = wal.log(WalRecord::Begin { txn_id }).unwrap();
    wal.flush().unwrap();
    let lsn2 = wal.log(WalRecord::Commit { txn_id }).unwrap();

    assert!(lsn2 > lsn1);
    assert_eq!(lsn2, Lsn::new(2));
}

#[test]
fn test_open_existing_wal() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();

    {
        let mut wal = WalManager::create(&path).unwrap();
        wal.log(WalRecord::Begin {
            txn_id: TransactionId::new(1),
        })
        .unwrap();
        wal.flush().unwrap();
    }

    // Reopen
    let wal = WalManager::open(&path).unwrap();
    assert!(wal.current_lsn().value() > 0);
}

#[test]
fn test_scan_reads_all_records() {
    let temp = NamedTempFile::new().unwrap();
    let mut wal = WalManager::create(temp.path()).unwrap();

    let txn_id = TransactionId::new(1);

    // Log several records
    wal.log(WalRecord::Begin { txn_id }).unwrap();
    wal.log(WalRecord::Insert {
        txn_id,
        relation_name: "test".to_string(),
        tuple_data: vec![1, 2, 3],
    })
    .unwrap();
    wal.log(WalRecord::Commit { txn_id }).unwrap();

    // Scan should return all records
    let records = wal.scan().unwrap();

    assert_eq!(records.len(), 3);
    assert!(matches!(records[0].1, WalRecord::Begin { .. }));
    assert!(matches!(records[1].1, WalRecord::Insert { .. }));
    assert!(matches!(records[2].1, WalRecord::Commit { .. }));
}

#[test]
fn test_wal_record_length_overflow() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();

    // Create a WAL file with a malicious record length
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();

        use std::io::Write; // Import Write trait

        // Write magic header
        file.write_all(WAL_MAGIC).unwrap();

        // Write a record: LSN=1, Len=u64::MAX
        // On 64-bit systems, u64::MAX fits in usize, but offset + len overflows.
        // On 32-bit systems, u64::MAX exceeds usize::MAX, so try_from fails early.
        let lsn = 1u64;
        let bad_len = u64::MAX;

        file.write_all(&lsn.to_le_bytes()).unwrap();
        file.write_all(&bad_len.to_le_bytes()).unwrap();

        // Write a few bytes of "data"
        file.write_all(&[1, 2, 3]).unwrap();
    }

    // Open calls scan_for_last_lsn internally.
    // It handles tail corruption by stopping, so it might return Ok.
    let result = WalManager::open(&path);

    let mut wal = match result {
        Ok(w) => w,
        Err(_) => return, // If it fails, that's also acceptable for this test
    };

    // However, explicit scan() should catch the corruption and return Error
    let scan_result = wal.scan();

    assert!(scan_result.is_err());
    match scan_result {
        Err(WalError::Corrupted(_, msg)) => {
            // We expect "exceeds memory limits" (32-bit)
            // OR "Record length causes offset overflow" (64-bit checked)
            // OR "Record extends beyond file"
            assert!(
                msg.contains("exceeds memory limits")
                    || msg.contains("Record length causes offset overflow")
                    || msg.contains("Record extends beyond file"),
                "Unexpected error message: {}",
                msg
            );
        }
        Ok(_) => panic!("Expected WalError::Corrupted, got Ok"),
        Err(e) => panic!("Expected WalError::Corrupted, got {:?}", e),
    }
}
