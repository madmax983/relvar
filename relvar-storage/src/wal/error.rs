//! WAL error types.

use thiserror::Error;

/// Errors that can occur during WAL operations.
#[derive(Debug, Error)]
/// Errors that can occur during Write-Ahead Logging operations.
///
/// This includes I/O failures, serialization issues, or unexpected EOF
/// errors during crash recovery.
///
/// # Examples
///
/// ```ignore
/// use relvar_storage::wal::error::WalError;
/// // Used throughout the WAL subsystem for robust error reporting.
/// ```
pub enum WalError {
    /// I/O error during WAL operations.
    #[error("WAL I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Record serialization error.
    #[error("WAL record error: {0}")]
    Record(#[from] super::record::WalRecordError),

    /// WAL is corrupted and cannot be read.
    #[error("WAL corrupted at LSN {0}: {1}")]
    Corrupted(super::lsn::Lsn, String),

    /// Attempted to read beyond end of WAL.
    #[error("End of WAL reached")]
    EndOfLog,

    /// Buffer is full and needs to be flushed.
    #[error("WAL buffer full ({0} bytes)")]
    BufferFull(usize),

    /// General WAL error.
    #[error("WAL error: {0}")]
    Other(String),
}
