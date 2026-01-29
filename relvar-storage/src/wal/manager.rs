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

        // Scan to end to find current LSN
        let file_len = log_file.seek(SeekFrom::End(0))?;
        let current_lsn = Lsn::new((file_len - WAL_MAGIC.len() as u64) / 8); // Rough estimate, will be fixed during recovery

        Ok(Self {
            log_file,
            current_lsn,
            buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
            buffer_capacity: DEFAULT_BUFFER_SIZE,
            flush_lsn: Lsn::new(0),
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
        if self.buffer.len() + serialized.len() + 8 > self.buffer_capacity {
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
}
