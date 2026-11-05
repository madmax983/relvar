//! Exclusive ownership of a database directory.
//!
//! Two engines on one directory would interleave WAL appends and heap page
//! writes (corrupting both), hand out the same transaction ids, and each
//! keep its own in-memory key index (breaking key uniqueness). Every
//! [`PersistentEngine`](super::PersistentEngine) therefore holds an
//! exclusive OS file lock on `<dir>/LOCK` for its whole lifetime.

use relvar_core::storage_engine::StorageError;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// Name of the lock file inside a database directory.
pub(crate) const LOCK_FILE_NAME: &str = "LOCK";

/// An exclusive lock on a database directory, released on drop.
///
/// Backed by [`File::try_lock`] (`flock` on Unix, `LockFileEx` on
/// Windows). The lock belongs to the open file handle, not to the
/// process, so two engines in one process conflict exactly like two
/// processes do, and the OS releases it when the handle closes — also when
/// the process crashes, so there is never a stale lock to clean up.
///
/// The `LOCK` file itself is deliberately left in place on drop: deleting
/// it would let a second opener lock a fresh file (a new inode) while a
/// third still holds the old one, so two engines could own the directory.
#[derive(Debug)]
pub(crate) struct DirectoryLock {
    /// The locked handle; dropping it releases the lock.
    _file: File,
}

impl DirectoryLock {
    /// Acquires the exclusive lock on `dir` without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Other`] naming the directory when another
    /// engine (in this or another process) holds the lock, or when the
    /// lock file cannot be opened or locked.
    pub(crate) fn acquire(dir: &Path) -> Result<Self, StorageError> {
        let path = dir.join(LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| {
                StorageError::Other(format!("failed to open lock file {}: {e}", path.display()))
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(StorageError::Other(format!(
                "database directory {} is locked by another engine \
                 (another process, or another open handle in this process)",
                dir.display()
            ))),
            Err(TryLockError::Error(e)) => Err(StorageError::Other(format!(
                "failed to lock database directory {}: {e}",
                dir.display()
            ))),
        }
    }
}
