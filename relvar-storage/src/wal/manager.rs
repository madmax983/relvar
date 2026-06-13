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
mod tests;
