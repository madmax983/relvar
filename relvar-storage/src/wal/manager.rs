//! WAL manager for coordinating log writes and flushes.
//!
//! The WalManager maintains a buffer of log records and coordinates their
//! writing to disk. It provides group commit functionality to batch multiple
//! log records into a single fsync operation.
//!
//! Two on-disk formats are supported:
//!
//! - **V2** (current): magic `RELVAR02`; frames are
//!   `[lsn: u64 LE][payload_len: u64 LE][postcard payload][crc32: u32 LE]`.
//!   The CRC detects torn writes. New files are always created as V2.
//! - **V1** (pre-v0.7): magic `RELVAR01`; frames are
//!   `[lsn: u64 LE][payload_len: u64 LE][postcard payload]` with no CRC.
//!   V1 files stay readable *and* appendable: the manager keeps writing
//!   whichever format the file already uses, so a file never mixes layouts.

use super::error::WalError;
use super::{
    Lsn, WAL_MAGIC_V1, WAL_MAGIC_V2, WalRecord, decode_frames, decode_legacy_frames, encode_frame,
};
use relvar_storage_core::wal::WalError as CoreWalError;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

/// Default WAL buffer size (1MB).
///
/// This allows batching many small records before flushing to disk.
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;

/// Maximum allowed WAL file size (2GB) before rotation is required to prevent OOM.
pub const MAX_WAL_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// On-disk WAL format, detected from the file's magic header.
///
/// New files are always created as [`WalFormat::V2`]. V1 files (written
/// before v0.7) stay appendable: the manager keeps writing whichever
/// format the file already uses, so a file never mixes frame layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalFormat {
    /// Pre-v0.7: magic `RELVAR01`, `[lsn][len][postcard]` frames, no CRC.
    V1,
    /// Current: magic `RELVAR02`, `[lsn][len][postcard][crc32]` frames.
    V2,
}

/// Maps a framing error from the storage core to the host [`WalError`].
///
/// Serialization failures keep the precise `Record` variant; anything else
/// (unreachable on encode, but the type allows it) becomes `Corrupted`.
fn map_frame_error(lsn: Lsn, error: CoreWalError) -> WalError {
    match error {
        CoreWalError::Record(record_error) => WalError::Record(record_error),
        other => WalError::Corrupted(lsn, other.to_string()),
    }
}

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

    /// On-disk format of this file (V1 legacy or V2 CRC-framed).
    format: WalFormat,

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
    /// New files always use the V2 format (magic `RELVAR02`, CRC-framed).
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
        log_file.write_all(WAL_MAGIC_V2)?;
        log_file.sync_all()?;

        Ok(Self {
            log_file,
            format: WalFormat::V2,
            current_lsn: Lsn::new(1),
            buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
            buffer_capacity: DEFAULT_BUFFER_SIZE,
            flush_lsn: Lsn::new(0),
        })
    }

    /// Opens an existing WAL file.
    ///
    /// Detects the on-disk format from the magic header: `RELVAR02` files
    /// use CRC-framed V2 records, `RELVAR01` files use legacy V1 framing.
    /// Appending to a V1 file keeps writing V1 frames (formats are never
    /// mixed within a file).
    ///
    /// # Errors
    ///
    /// Returns `WalError::Io` if file open fails.
    /// Returns `WalError::Corrupted` if the header is invalid.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, WalError> {
        let mut log_file = OpenOptions::new().read(true).write(true).open(path)?;

        // Verify magic header and detect format
        let mut magic = [0u8; 8];
        use std::io::Read;
        log_file.read_exact(&mut magic)?;

        let format = if &magic == WAL_MAGIC_V1 {
            WalFormat::V1
        } else if &magic == WAL_MAGIC_V2 {
            WalFormat::V2
        } else {
            return Err(WalError::Corrupted(
                Lsn::new(0),
                "Invalid WAL magic header".to_string(),
            ));
        };

        // Scan WAL to find the actual last LSN
        // CRITICAL: Can't use file_len heuristic - records are variable length!
        let (current_lsn, flush_lsn) = Self::scan_for_last_lsn(&mut log_file, format)?;

        Ok(Self {
            log_file,
            format,
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
    /// The record is framed in the file's format: V2 files get
    /// CRC-protected frames via the storage core; V1 files keep the legacy
    /// `[lsn][len][postcard]` framing so formats are never mixed.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Record` if serialization fails.
    /// Returns `WalError::Io` if flush fails.
    pub fn log(&mut self, record: WalRecord) -> Result<Lsn, WalError> {
        let lsn = self.current_lsn;

        // Frame the record in the file's format (never mix V1 and V2).
        let frame = match self.format {
            WalFormat::V1 => {
                let serialized = record.serialize()?;
                let mut frame = Vec::with_capacity(16 + serialized.len());
                frame.extend_from_slice(&lsn.value().to_le_bytes());
                frame.extend_from_slice(&(serialized.len() as u64).to_le_bytes());
                frame.extend_from_slice(&serialized);
                frame
            }
            WalFormat::V2 => encode_frame(lsn, &record).map_err(|e| map_frame_error(lsn, e))?,
        };

        // Auto-flush if the buffer would overflow.
        if self
            .buffer
            .len()
            .checked_add(frame.len())
            .is_none_or(|len| len > self.buffer_capacity)
        {
            self.flush()?;
        }

        // Assign LSN
        self.current_lsn = self.current_lsn.next();

        // Buffer the framed record.
        self.buffer.extend_from_slice(&frame);

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
    /// and deserializes all records. Used during recovery. Frames are
    /// decoded with the storage core: V2 files get CRC-checked decoding,
    /// V1 files the legacy framing.
    ///
    /// # Errors
    ///
    /// Returns `WalError::Io` if reading fails.
    /// Returns `WalError::Corrupted` if a record cannot be deserialized.
    pub fn scan(&mut self) -> Result<Vec<(Lsn, WalRecord)>, WalError> {
        let buffer = self.read_record_bytes()?;

        // Both decoders yield the same item type; box them to share the loop.
        let decoder: Box<dyn Iterator<Item = Result<(Lsn, WalRecord), CoreWalError>>> =
            match self.format {
                WalFormat::V1 => Box::new(decode_legacy_frames(&buffer)),
                WalFormat::V2 => Box::new(decode_frames(&buffer)),
            };

        let mut records = Vec::new();
        let mut last_lsn = Lsn::new(0);
        for item in decoder {
            let (lsn, record) = item.map_err(|e| WalError::Corrupted(last_lsn, e.to_string()))?;
            last_lsn = lsn;
            records.push((lsn, record));
        }

        Ok(records)
    }

    /// Reads the raw record bytes after the magic header into memory.
    ///
    /// Flushes buffered records first and seeks back to the end afterwards,
    /// so the file stays positioned for future appends.
    fn read_record_bytes(&mut self) -> Result<Vec<u8>, WalError> {
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

        // Seek to start of records (after magic header; both magics are 8 bytes)
        self.log_file.seek(SeekFrom::Start(8))?;

        let mut buffer = Vec::new();

        // Read entire file into buffer, capped at 2GB to prevent unbounded allocation DoS
        (&mut self.log_file)
            .take(MAX_WAL_SIZE)
            .read_to_end(&mut buffer)?;

        // Seek back to end for future writes
        self.log_file.seek(SeekFrom::End(0))?;

        Ok(buffer)
    }

    /// Scans the WAL file to find the last LSN.
    ///
    /// Returns (next_lsn, last_flushed_lsn) by parsing the entire WAL.
    /// This is necessary because WAL records are variable-length.
    /// A corrupt/torn tail ends the scan at the last valid record.
    fn scan_for_last_lsn(log_file: &mut File, format: WalFormat) -> Result<(Lsn, Lsn), WalError> {
        use std::io::Read;

        // Seek to start of records (after magic header; both magics are 8 bytes)
        log_file.seek(SeekFrom::Start(8))?;

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

        let decoder: Box<dyn Iterator<Item = Result<(Lsn, WalRecord), CoreWalError>>> = match format
        {
            WalFormat::V1 => Box::new(decode_legacy_frames(&buffer)),
            WalFormat::V2 => Box::new(decode_frames(&buffer)),
        };

        let mut last_lsn = Lsn::new(0);
        for item in decoder {
            // If decoding fails, we treat it as end of valid log (break)
            if let Ok((lsn, _)) = item {
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
    fn test_v2_roundtrip_crc_framing() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let txn_id = TransactionId::new(1);
        let records_to_log = [
            WalRecord::Begin { txn_id },
            WalRecord::Insert {
                txn_id,
                relation_name: "test".to_string(),
                tuple_data: vec![1, 2, 3],
            },
            WalRecord::Commit { txn_id },
        ];
        {
            let mut wal = WalManager::create(&path).unwrap();
            for record in &records_to_log {
                wal.log(record.clone()).unwrap();
            }
            wal.flush().unwrap();
        }

        // New files use the V2 magic, and each frame carries a 4-byte CRC:
        // file = magic(8) + sum( lsn(8) + len(8) + payload + crc(4) ).
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(&raw[..8], WAL_MAGIC_V2);
        let expected_len: usize = 8 + records_to_log
            .iter()
            .map(|r| 8 + 8 + r.serialize().unwrap().len() + 4)
            .sum::<usize>();
        assert_eq!(raw.len(), expected_len);

        // Reopen and scan: all records come back with their LSNs.
        let mut wal = WalManager::open(&path).unwrap();
        let records = wal.scan().unwrap();
        assert_eq!(records.len(), 3);
        for (i, (lsn, record)) in records.iter().enumerate() {
            assert_eq!(*lsn, Lsn::new(i as u64 + 1));
            assert_eq!(record, &records_to_log[i]);
        }
    }

    #[test]
    fn test_v2_crc_detects_corruption() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let txn_id = TransactionId::new(1);
        {
            let mut wal = WalManager::create(&path).unwrap();
            wal.log(WalRecord::Begin { txn_id }).unwrap();
            wal.flush().unwrap();
        }

        // Corrupt one payload byte (after magic + lsn + len).
        {
            let mut raw = std::fs::read(&path).unwrap();
            let payload_offset = 8 + 8 + 8;
            raw[payload_offset] ^= 0xFF;
            std::fs::write(&path, &raw).unwrap();
        }

        let mut wal = WalManager::open(&path).unwrap();
        let result = wal.scan();
        assert!(result.is_err());
        match result {
            Err(WalError::Corrupted(_, msg)) => {
                assert!(msg.contains("CRC mismatch"), "unexpected: {msg}");
            }
            other => panic!("expected Corrupted, got {other:?}"),
        }
    }

    /// Hand-writes a V1 (pre-v0.7) WAL file: magic `RELVAR01` followed by
    /// legacy `[lsn:8][len:8][postcard]` frames with no CRC.
    fn write_v1_wal(path: &std::path::Path, records: &[(u64, WalRecord)]) {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .unwrap();
        file.write_all(WAL_MAGIC_V1).unwrap();
        for (lsn, record) in records {
            let payload = record.serialize().unwrap();
            file.write_all(&lsn.to_le_bytes()).unwrap();
            file.write_all(&(payload.len() as u64).to_le_bytes())
                .unwrap();
            file.write_all(&payload).unwrap();
        }
    }

    #[test]
    fn test_v1_hand_written_compat() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let txn_id = TransactionId::new(1);
        let logged = [
            (1u64, WalRecord::Begin { txn_id }),
            (
                2u64,
                WalRecord::Insert {
                    txn_id,
                    relation_name: "test".to_string(),
                    tuple_data: vec![7, 8, 9],
                },
            ),
        ];
        write_v1_wal(&path, &logged);

        // V1 files open and scan without the CRC layer.
        let mut wal = WalManager::open(&path).unwrap();
        assert_eq!(wal.current_lsn(), Lsn::new(3));
        let records = wal.scan().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].0, Lsn::new(1));
        assert_eq!(records[0].1, logged[0].1);
        assert_eq!(records[1].0, Lsn::new(2));
        assert_eq!(records[1].1, logged[1].1);

        // Appending to a V1 file continues V1 framing: no CRC bytes appear,
        // so the file never mixes layouts.
        wal.log(WalRecord::Commit { txn_id }).unwrap();
        wal.flush().unwrap();
        drop(wal);

        let raw = std::fs::read(&path).unwrap();
        let commit_payload = WalRecord::Commit { txn_id }.serialize().unwrap();
        let expected_len: usize = 8
            + logged
                .iter()
                .map(|(_, r)| 8 + 8 + r.serialize().unwrap().len())
                .sum::<usize>()
            + (8 + 8 + commit_payload.len());
        assert_eq!(raw.len(), expected_len);

        let mut wal = WalManager::open(&path).unwrap();
        let records = wal.scan().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[2].0, Lsn::new(3));
        assert!(matches!(records[2].1, WalRecord::Commit { .. }));
    }

    #[test]
    fn test_v1_corrupt_payload_fails_scan() {
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

            // Write V1 magic header
            file.write_all(WAL_MAGIC_V1).unwrap();

            // Write a validly framed record but with an un-deserializable payload
            let lsn = 1u64;
            let bad_payload = vec![0xFF; 20];
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
                assert!(msg.contains("WAL record error"), "unexpected: {msg}");
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
            file.write_all(WAL_MAGIC_V1).unwrap();

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
            file.write_all(WAL_MAGIC_V1).unwrap();

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
                // The legacy decoder rejects the absurd length before reading:
                // "WAL frame payload length <u64::MAX> exceeds maximum record size".
                assert!(
                    msg.contains("exceeds maximum record size"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            Ok(_) => panic!("Expected WalError::Corrupted, got Ok"),
            Err(e) => panic!("Expected WalError::Corrupted, got {:?}", e),
        }
    }
}
