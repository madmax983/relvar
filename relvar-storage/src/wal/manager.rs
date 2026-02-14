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
/// # Example
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
/// ```
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
    /// Returns the LSN assigned to this record. If the buffer is full,
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
        if self.buffer.len() + serialized.len() + 16 > self.buffer_capacity {
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

    /// Returns the current LSN (next to be assigned).
    pub fn current_lsn(&self) -> Lsn {
        self.current_lsn
    }

    /// Returns the LSN of the last flushed record.
    pub fn flush_lsn(&self) -> Lsn {
        self.flush_lsn
    }

    /// Returns the current buffer size in bytes.
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
        use std::io::Read;

        // Flush any buffered records first
        self.flush()?;

        // Seek to start of records (after magic header)
        self.log_file
            .seek(SeekFrom::Start(WAL_MAGIC.len() as u64))?;

        let mut records = Vec::new();
        let mut valid_end_pos = WAL_MAGIC.len() as u64;

        loop {
            // Check if we hit EOF or partial header (treat as end of log)
            let (lsn, record_len) = match Self::read_header(&mut self.log_file) {
                Ok(Some(val)) => val,
                Ok(None) => break, // Clean EOF
                Err(WalError::Corrupted(_, msg)) if msg.contains("Unexpected EOF") => {
                    // Partial header at end of file - treat as end of log for recovery
                    break;
                }
                Err(e) => return Err(e),
            };

            // Allocate buffer for record data
            let mut record_buffer = vec![0u8; record_len as usize];

            // Read record data
            match self.log_file.read_exact(&mut record_buffer) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // Partial record data at end of file - treat as end of log
                    break;
                }
                Err(e) => return Err(WalError::Io(e)),
            }

            let record = WalRecord::deserialize(&record_buffer)
                .map_err(|e| WalError::Corrupted(lsn, format!("Deserialization failed: {}", e)))?;

            records.push((lsn, record));
            valid_end_pos = self.log_file.stream_position()?;
        }

        // Seek back to end of valid data for future writes
        self.log_file.seek(SeekFrom::Start(valid_end_pos))?;

        Ok(records)
    }

    /// Scans the WAL file to find the last LSN.
    ///
    /// Returns (next_lsn, last_flushed_lsn) by parsing the entire WAL.
    /// This is necessary because WAL records are variable-length.
    fn scan_for_last_lsn(log_file: &mut File) -> Result<(Lsn, Lsn), WalError> {
        // Seek to start of records (after magic header)
        log_file.seek(SeekFrom::Start(WAL_MAGIC.len() as u64))?;

        let mut last_lsn = Lsn::new(0);
        let mut valid_end_pos = WAL_MAGIC.len() as u64;
        let file_len = log_file.metadata()?.len();

        loop {
            // Check if we hit EOF or partial header (treat as end of log)
            let (lsn, record_len) = match Self::read_header(log_file) {
                Ok(Some(val)) => val,
                Ok(None) => break, // Clean EOF
                Err(WalError::Corrupted(_, msg)) if msg.contains("Unexpected EOF") => {
                    // Partial header at end of file - treat as end of log
                    break;
                }
                Err(e) => return Err(e),
            };

            // Validate payload existence before seeking to avoid creating holes
            let current_pos = log_file.stream_position()?;

            if current_pos + record_len > file_len {
                // Partial record extends beyond file - treat as end of log
                break;
            }

            // Skip record data
            log_file.seek(SeekFrom::Current(record_len as i64))?;

            // Update last valid state
            last_lsn = lsn;
            valid_end_pos = current_pos + record_len;
        }

        // Seek to the end of the last valid record to overwrite any partial garbage
        log_file.seek(SeekFrom::Start(valid_end_pos))?;

        // Next LSN is last_lsn + 1
        let next_lsn = last_lsn.next();

        // All scanned records are flushed (they're on disk)
        let flush_lsn = last_lsn;

        Ok((next_lsn, flush_lsn))
    }

    /// Helper to read a record header from the file.
    fn read_header(file: &mut File) -> Result<Option<(Lsn, u64)>, WalError> {
        use std::io::Read;
        let mut header_buffer = [0u8; 16];

        // Read potentially partial header
        let mut bytes_read = 0;
        while bytes_read < 16 {
            match file.read(&mut header_buffer[bytes_read..]) {
                Ok(0) => {
                    if bytes_read == 0 {
                        return Ok(None); // Clean EOF
                    } else {
                        return Err(WalError::Corrupted(
                            Lsn::new(0),
                            "Unexpected EOF in header".to_string(),
                        ));
                    }
                }
                Ok(n) => bytes_read += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(WalError::Io(e)),
            }
        }

        // Parse LSN (8 bytes)
        let lsn_bytes: [u8; 8] = header_buffer[0..8].try_into().unwrap();
        let lsn = Lsn::new(u64::from_le_bytes(lsn_bytes));

        // Parse record length (8 bytes)
        let len_bytes: [u8; 8] = header_buffer[8..16].try_into().unwrap();
        let record_len = u64::from_le_bytes(len_bytes);

        // Sanity check length
        if record_len > super::record::MAX_RECORD_SIZE as u64 {
            return Err(WalError::Corrupted(
                lsn,
                format!(
                    "Record too large: {} bytes (max: {})",
                    record_len,
                    super::record::MAX_RECORD_SIZE
                ),
            ));
        }

        Ok(Some((lsn, record_len)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wal::TransactionId;
    use tempfile::NamedTempFile;

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
}

#[cfg(test)]
mod incremental_scan_tests {
    use super::*;
    use crate::wal::TransactionId;
    use tempfile::NamedTempFile;

    #[test]
    fn test_wal_scan_incremental_read_logic() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        // 1. Create WAL and write multiple records
        let mut wal = WalManager::create(path).unwrap();
        let txn_id = TransactionId::new(1);

        // Write a sequence of records
        wal.log(WalRecord::Begin { txn_id }).unwrap();

        // Write enough records to verify loop correctness
        for i in 0..100 {
            wal.log(WalRecord::Insert {
                txn_id,
                relation_name: "test_rel".to_string(),
                tuple_data: vec![i as u8; 100], // 100 bytes each
            })
            .unwrap();
        }

        wal.log(WalRecord::Commit { txn_id }).unwrap();
        wal.flush().unwrap();

        // 2. Open and Scan (new instance)
        let mut wal_scan = WalManager::open(path).unwrap();
        let records = wal_scan.scan().unwrap();

        // Verify count: 1 Begin + 100 Inserts + 1 Commit = 102
        assert_eq!(records.len(), 102);

        // Verify content
        assert!(matches!(records[0].1, WalRecord::Begin { .. }));

        for i in 0..100 {
            let record = &records[i + 1].1;
            match record {
                WalRecord::Insert { tuple_data, .. } => {
                    assert_eq!(tuple_data.len(), 100);
                    assert_eq!(tuple_data[0], i as u8);
                }
                _ => panic!("Expected Insert at index {}", i),
            }
        }

        assert!(matches!(records[101].1, WalRecord::Commit { .. }));
    }
}
