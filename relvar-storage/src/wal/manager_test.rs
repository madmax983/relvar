//! Dummy file to fix tarpaulin coverage metric issues
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
