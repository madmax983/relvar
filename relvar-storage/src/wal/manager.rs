//! WAL manager for coordinating log writes and flushes.
//!
//! The WalManager maintains a buffer of log records and coordinates their
//! writing to disk. It provides group commit functionality to batch multiple
//! log records into a single fsync operation.

use super::error::WalError;
use super::lsn::Lsn;
use super::record::WalRecord;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

/// Default WAL buffer size (1MB).
///
/// This allows batching many small records before flushing to disk.
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;

/// Maximum allowed WAL file size (2GB) before rotation is required to prevent OOM.
pub const MAX_WAL_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// Header written at the start of each WAL file.
///
/// Used for basic validation and version checking during recovery.
const WAL_MAGIC: &[u8; 8] = b"RELVAR01";

/// Manages write-ahead logging operations.
///
/// The WalManager coordinates logging, buffering, and flushing of WAL records.
/// It implements a simple single-buffer design with automatic flushing when
/// the buffer reaches capacity.
///
/// # Examples
///
/// ```ignore
/// use relvar_storage::wal::WalManager;
/// use relvar_storage::wal::WalRecord;
/// use relvar_storage::wal::TransactionId;
///
/// let mut wal = WalManager::create("data.wal").unwrap();
///
/// // Log a transaction begin
/// let txn_id = TransactionId::new(1);
/// let lsn = wal.log(WalRecord::Begin { txn_id }).unwrap();
///
/// // Flush ensures durability
/// wal.flush().unwrap();
/// ```ignore
pub struct WalManager {
    /// The log file.
    log_file: File,

    /// Current LSN (next to be assigned).
    current_lsn: Lsn,

    /// In-memory buffer for log records.
    buffer: Vec<u8>,

    /// Maximum buffer size before automatic flush.
    buffer_capacity: usize,

    /// LSN of the last flushed record.
    flush_lsn: Lsn,
}

impl WalManager {
    /// Creates a new WAL file and initializes the manager.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Io` if file creation or header write fails.
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, WalError> {
        let mut log_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        // Write magic header
        log_file.write_all(WAL_MAGIC)?;
        log_file.sync_all()?;

        Ok(Self {
            log_file,
            current_lsn: Lsn::new(1),
            buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
            buffer_capacity: DEFAULT_BUFFER_SIZE,
            flush_lsn: Lsn::new(0),
        })
    }

    /// Opens an existing WAL file.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Io` if file open fails.
    /// Returns `WalError::Corrupted` if the header is invalid.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, WalError> {
        let mut log_file = OpenOptions::new().read(true).write(true).open(path)?;

        // Verify magic header
        let mut magic = [0u8; 8];
        use std::io::Read;
        log_file.read_exact(&mut magic)?;

        if &magic != WAL_MAGIC {
            return Err(WalError::Corrupted(
                Lsn::new(0),
                "Invalid WAL magic header".to_string(),
            ));
        }

        // Scan WAL to find the actual last LSN
        // CRITICAL: Can't use file_len heuristic - records are variable length!
        let (current_lsn, flush_lsn) = Self::scan_for_last_lsn(&mut log_file)?;

        Ok(Self {
            log_file,
            current_lsn,
            buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
            buffer_capacity: DEFAULT_BUFFER_SIZE,
            flush_lsn,
        })
    }

    /// Logs a record to the WAL buffer.
    ///
    /// Provides the newly assigned LSN for this record. If the buffer is full,
    /// it will be automatically flushed before adding the new record.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Record` if serialization fails.
    /// Returns `WalError::Io` if flush fails.
    pub fn log(&mut self, record: WalRecord) -> Result<Lsn, WalError> {
        let serialized = record.serialize()?;

        // Auto-flush if buffer would overflow
        // Each record writes: 8 bytes (LSN) + 8 bytes (length) + data
        let required_len = self
            .buffer
            .len()
            .checked_add(serialized.len())
            .and_then(|sum| sum.checked_add(16));

        if required_len.is_none_or(|len| len > self.buffer_capacity) {
            self.flush()?;
        }

        // Assign LSN
        let lsn = self.current_lsn;
        self.current_lsn = self.current_lsn.next();

        // Write record to buffer: [lsn (8 bytes)][record_len (8 bytes)][record_data]
        self.buffer.extend_from_slice(&lsn.value().to_le_bytes());
        self.buffer
            .extend_from_slice(&(serialized.len() as u64).to_le_bytes());
        self.buffer.extend_from_slice(&serialized);

        Ok(lsn)
    }

    /// Flushes the buffer to disk and performs fsync.
    ///
    /// All buffered records are written to the log file and synchronized
    /// to disk, ensuring durability.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Io` if write or sync fails.
    pub fn flush(&mut self) -> Result<(), WalError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Write buffer to file
        self.log_file.write_all(&self.buffer)?;

        // Ensure data is on disk (durability)
        self.log_file.sync_all()?;

        // Update flush LSN to last logged record
        self.flush_lsn = Lsn::new(self.current_lsn.value() - 1);

        // Clear buffer
        self.buffer.clear();

        Ok(())
    }

    /// Retrieves the current LSN, representing the next sequential identifier to be assigned.
    ///
    /// This is useful for checking the current logical position within the WAL
    /// before performing new log writes.
    pub fn current_lsn(&self) -> Lsn {
        self.current_lsn
    }

    /// Retrieves the LSN corresponding to the most recently flushed record.
    ///
    /// Any record with an LSN less than or equal to this value is guaranteed to
    /// be durably persisted to disk. This is heavily utilized during checkpoints.
    pub fn flush_lsn(&self) -> Lsn {
        self.flush_lsn
    }

    /// Retrieves the total size of the un-flushed log record buffer in bytes.
    ///
    /// This metric tracks how many bytes of WAL records are currently held in memory
    /// pending a sync to disk. Once it exceeds `buffer_capacity`, a flush is triggered automatically.
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the buffer is empty.
    pub fn is_buffer_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Scans the WAL file and yields all records in order.
    ///
    /// This reads from the beginning of the WAL file (after the header)
    /// and deserializes all records. Used during recovery.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Io` if reading fails.
    /// Returns `WalError::Corrupted` if a record cannot be deserialized.
    pub fn scan(&mut self) -> Result<Vec<(Lsn, WalRecord)>, WalError> {
        use crate::wal::iter::WalRecordIter;
        use std::io::Read;

        // Flush any buffered records first
        self.flush()?;

        // Enforce maximum file size to prevent unbounded memory allocation
        let file_len = self.log_file.metadata()?.len();
        if file_len > MAX_WAL_SIZE {
            return Err(WalError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "WAL file too large: {} bytes (max {})",
                    file_len, MAX_WAL_SIZE
                ),
            )));
        }

        // Seek to start of records (after magic header)
        self.log_file
            .seek(SeekFrom::Start(WAL_MAGIC.len() as u64))?;

        let mut records = Vec::new();
        let mut buffer = Vec::new();

        // Read entire file into buffer, capped at 2GB to prevent unbounded allocation DoS
        (&mut self.log_file)
            .take(MAX_WAL_SIZE)
            .read_to_end(&mut buffer)?;

        for item in WalRecordIter::new(&buffer) {
            let (lsn, _, record_bytes) = item?;
            let record = WalRecord::deserialize(record_bytes)
                .map_err(|e| WalError::Corrupted(lsn, format!("Deserialization failed: {}", e)))?;
            records.push((lsn, record));
        }

        // Seek back to end for future writes
        self.log_file.seek(SeekFrom::End(0))?;

        Ok(records)
    }

    /// Scans the WAL file to find the last LSN.
    ///
    /// Returns (next_lsn, last_flushed_lsn) by parsing the entire WAL.
    /// This is necessary because WAL records are variable-length.
    fn scan_for_last_lsn(log_file: &mut File) -> Result<(Lsn, Lsn), WalError> {
        use crate::wal::iter::WalRecordIter;
        use std::io::Read;

        // Seek to start of records (after magic header)
        log_file.seek(SeekFrom::Start(WAL_MAGIC.len() as u64))?;

        // Enforce maximum file size to prevent unbounded memory allocation
        let file_len = log_file.metadata()?.len();
        if file_len > MAX_WAL_SIZE {
            return Err(WalError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "WAL file too large: {} bytes (max {})",
                    file_len, MAX_WAL_SIZE
                ),
            )));
        }

        let mut buffer = Vec::new();
        (&mut *log_file)
            .take(MAX_WAL_SIZE)
            .read_to_end(&mut buffer)?;

        let mut last_lsn = Lsn::new(0);

        for item in WalRecordIter::new(&buffer) {
            // If iteration fails, we treat it as end of valid log (break)
            if let Ok((lsn, _, _)) = item {
                last_lsn = lsn;
            } else {
                break;
            }
        }

        // Next LSN is last_lsn + 1
        let next_lsn = last_lsn.next();

        // All scanned records are flushed (they're on disk)
        let flush_lsn = last_lsn;

        Ok((next_lsn, flush_lsn))
    }
}

#[cfg(test)]
mod tests {
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
}
