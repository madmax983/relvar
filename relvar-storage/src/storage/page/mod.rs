//! Page-based I/O for fixed-size disk blocks.
//!
//! This module provides the lowest-level storage abstraction: fixed-size pages
//! that serve as the unit of I/O between disk and memory. All higher-level
//! storage structures (heap files, indexes) are built on top of pages.
//!
//! # Page Layout
//!
//! Each page on disk has the following format:
//!
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │ data_length (8 bytes, little-endian u64)           │
//! ├────────────────────────────────────────────────────┤
//! │ actual_data (variable, up to PAGE_SIZE - 8 bytes)  │
//! ├────────────────────────────────────────────────────┤
//! │ padding (zeros to fill PAGE_SIZE)                  │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```no_run
//! use relvar_storage::storage::{Page, PageFile, PAGE_SIZE};
//!
//! // Create a page file
//! let mut pf = PageFile::create("data.pages").unwrap();
//!
//! // Create and write a page
//! let page = Page::from_data(0, vec![1, 2, 3, 4, 5]).unwrap();
//! pf.write_page(&page).unwrap();
//!
//! // Read it back
//! let loaded = pf.read_page(0).unwrap();
//! assert_eq!(loaded.data(), &[1, 2, 3, 4, 5]);
//! ```

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

/// Page size in bytes (4KB).
///
/// This size (4096 bytes) is chosen to align with:
/// - Standard disk sector sizes (typically 4KB on modern SSDs/HDDs).
/// - Virtual memory page sizes (typically 4KB on x86/ARM).
///
/// # Trade-offs
/// - **I/O Efficiency:** Matches the atomic write unit of most storage hardware, minimizing write amplification.
/// - **Fragmentation:** Smaller pages reduce wasted space (internal fragmentation) for small tuples but increase metadata overhead.
/// - **Memory Usage:** 4KB is small enough to keep many pages cached in memory.
pub const PAGE_SIZE: usize = 4096;

/// Unique identifier for a page within a page file.
///
/// Pages are numbered sequentially starting from 0. The page ID determines
/// the byte offset in the file: `offset = page_id * PAGE_SIZE`.
pub type PageId = u64;

/// Errors that can occur during page operations.
#[derive(Debug, Error)]
pub enum PageError {
    /// An I/O error occurred while reading or writing.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The page data exceeds the maximum allowed size.
    #[error("Page data exceeds maximum size")]
    PageTooLarge,
}

/// A fixed-size block of data stored on disk.
///
/// Pages are the fundamental unit of I/O between disk and memory. Each page
/// has a unique ID and can hold up to [`PAGE_SIZE`] bytes of data. The actual
/// data is stored along with its length to handle variable-size content.
///
/// # Examples
///
/// ```
/// use relvar_storage::storage::{Page, PAGE_SIZE};
///
/// // Create an empty page
/// let mut page = Page::new(0);
/// assert!(page.is_empty());
/// assert_eq!(page.available_space(), PAGE_SIZE);
///
/// // Create a page with data
/// let page = Page::from_data(1, vec![1, 2, 3, 4, 5]).unwrap();
/// assert_eq!(page.data().len(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// Page identifier.
    id: PageId,
    /// Raw page data (up to PAGE_SIZE bytes).
    data: Vec<u8>,
}

impl Page {
    /// Creates a new empty page with the given ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier for this page
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage::storage::Page;
    ///
    /// let page = Page::new(0);
    /// assert_eq!(page.id(), 0);
    /// assert!(page.is_empty());
    /// ```
    pub fn new(id: PageId) -> Self {
        Self {
            id,
            data: Vec::new(),
        }
    }

    /// Creates a page with the given ID and data.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier for this page
    /// * `data` - The data to store in the page
    ///
    /// # Errors
    ///
    /// Returns [`PageError::PageTooLarge`] if `data.len() > PAGE_SIZE`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage::storage::Page;
    ///
    /// let page = Page::from_data(42, vec![1, 2, 3]).unwrap();
    /// assert_eq!(page.id(), 42);
    /// assert_eq!(page.data(), &[1, 2, 3]);
    /// ```
    pub fn from_data(id: PageId, data: Vec<u8>) -> Result<Self, PageError> {
        // Limit to PAGE_SIZE - 8 to account for the 8-byte length prefix
        // written by write_page(), ensuring total on-disk size is exactly PAGE_SIZE
        if data.len() > PAGE_SIZE - 8 {
            return Err(PageError::PageTooLarge);
        }
        Ok(Self { id, data })
    }

    /// Obtains the `PageId`, which serves as the physical address of this page on disk.
    ///
    /// The page ID corresponds to the offset in the page file (`page_id * PAGE_SIZE`).
    /// This is strictly for internal storage engine routing and is never exposed to
    /// the logical relational layer (TTM Proscription 6).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage::storage::Page;
    ///
    /// let page = Page::new(42);
    /// assert_eq!(page.id(), 42);
    /// ```
    pub fn id(&self) -> PageId {
        self.id
    }

    /// Exposes the underlying byte array of the page for reading.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Replaces the page's data.
    ///
    /// # Errors
    ///
    /// Returns [`PageError::PageTooLarge`] if `data.len() > PAGE_SIZE`.
    pub fn set_data(&mut self, data: Vec<u8>) -> Result<(), PageError> {
        if data.len() > PAGE_SIZE {
            return Err(PageError::PageTooLarge);
        }
        self.data = data;
        Ok(())
    }

    /// Calculates the remaining free space in bytes within this page.
    ///
    /// This is used by the `HeapFile` during inserts to quickly determine if a given
    /// serialized tuple can fit into the currently loaded page without needing to
    /// allocate a new one.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage::storage::{Page, PAGE_SIZE};
    ///
    /// let page = Page::new(1);
    /// // A new page starts completely empty (data length is 0).
    /// assert_eq!(page.available_space(), PAGE_SIZE);
    /// ```
    ///
    /// This is `PAGE_SIZE - data.len()`.
    pub fn available_space(&self) -> usize {
        PAGE_SIZE - self.data.len()
    }

    /// Returns `true` if the page contains no data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Manages fixed-size pages stored in a file on disk.
///
/// `PageFile` provides random access to pages by their ID. Pages are stored
/// at fixed offsets (`page_id * PAGE_SIZE`), allowing efficient seek-based
/// access without scanning the entire file.
///
/// # File Format
///
/// The file consists of consecutive PAGE_SIZE blocks:
///
/// ```text
/// Offset 0:                 Page 0
/// Offset PAGE_SIZE:         Page 1
/// Offset 2*PAGE_SIZE:       Page 2
/// ...
/// ```
///
/// # Examples
///
/// ```no_run
/// use relvar_storage::storage::{Page, PageFile};
///
/// // Create a new page file
/// let mut pf = PageFile::create("data.pages").unwrap();
///
/// // Write pages (can write in any order)
/// pf.write_page(&Page::from_data(0, vec![1, 2, 3]).unwrap()).unwrap();
/// pf.write_page(&Page::from_data(5, vec![4, 5, 6]).unwrap()).unwrap();
///
/// // Read pages back
/// let page0 = pf.read_page(0).unwrap();
/// let page5 = pf.read_page(5).unwrap();
/// ```
pub struct PageFile {
    /// The underlying file handle.
    file: File,
}

impl PageFile {
    /// Creates a new page file, truncating any existing file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the page file will be created
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Io`] if the file cannot be created.
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, PageError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Ok(Self { file })
    }

    /// Opens an existing page file, or creates one if it doesn't exist.
    ///
    /// Unlike [`create`](Self::create), this preserves existing content.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the page file
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Io`] if the file cannot be opened.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, PageError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        Ok(Self { file })
    }

    /// Reads a page from disk.
    ///
    /// Seeks to the page's offset (`page_id * PAGE_SIZE`) and reads the data.
    /// If the page doesn't exist or is unreadable, returns an empty page.
    ///
    /// # Arguments
    ///
    /// * `page_id` - The ID of the page to read
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Io`] if the read fails.
    pub fn read_page(&mut self, page_id: PageId) -> Result<Page, PageError> {
        let mut buffer = vec![0u8; PAGE_SIZE];
        let bytes_read = self.read_raw_page(page_id, &mut buffer)?;

        // If we read nothing, it's a new/empty page
        if bytes_read == 0 {
            return Ok(Page::new(page_id));
        }

        // Truncate to actual bytes read
        buffer.truncate(bytes_read);
        Self::parse_page_data(page_id, &buffer)
    }

    fn read_raw_page(&mut self, page_id: PageId, buffer: &mut [u8]) -> Result<usize, PageError> {
        let offset = page_id
            .checked_mul(PAGE_SIZE as u64)
            .ok_or(PageError::PageTooLarge)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read(buffer).map_err(PageError::Io)
    }

    fn parse_page_data(page_id: PageId, buffer: &[u8]) -> Result<Page, PageError> {
        // We need at least 8 bytes for the length prefix
        if buffer.len() < 8 {
            return Err(PageError::Serialization(format!(
                "Page too short: {} bytes",
                buffer.len()
            )));
        }

        let data_len_u64 =
            u64::from_le_bytes(buffer[0..8].try_into().map_err(|_| {
                PageError::Serialization("Failed to parse length prefix".to_string())
            })?);

        // Check if data length exceeds PAGE_SIZE - 8 (maximum possible data)
        if data_len_u64 > (PAGE_SIZE - 8) as u64 {
            return Err(PageError::Serialization(format!(
                "Page data length {} exceeds maximum {}",
                data_len_u64,
                PAGE_SIZE - 8
            )));
        }

        let data_len = data_len_u64 as usize;
        let required_len = data_len + 8; // No overflow possible (checked above)

        if required_len > buffer.len() {
            return Err(PageError::Serialization(format!(
                "Corrupted page: length prefix says {}, but only {} bytes available",
                data_len,
                buffer.len()
            )));
        }

        let actual_data = buffer[8..8 + data_len].to_vec();
        Page::from_data(page_id, actual_data)
    }

    /// Writes a page to disk.
    ///
    /// The page is written at offset `page.id() * PAGE_SIZE`. The data is
    /// prefixed with its length and padded to fill exactly PAGE_SIZE bytes.
    ///
    /// # Arguments
    ///
    /// * `page` - The page to write
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Io`] if the write fails.
    pub fn write_page(&mut self, page: &Page) -> Result<(), PageError> {
        // Seek to the page offset
        let offset = page
            .id()
            .checked_mul(PAGE_SIZE as u64)
            .ok_or(PageError::PageTooLarge)?;
        self.file.seek(SeekFrom::Start(offset))?;

        // Prepare buffer with length prefix and data
        let mut buffer = Vec::with_capacity(PAGE_SIZE);

        // Write data length (8 bytes)
        let data_len = page.data().len() as u64;
        buffer.extend_from_slice(&data_len.to_le_bytes());

        // Write actual data
        buffer.extend_from_slice(page.data());

        // Ensure buffer doesn't exceed PAGE_SIZE (would corrupt page alignment)
        if buffer.len() > PAGE_SIZE {
            return Err(PageError::PageTooLarge);
        }

        // Pad to PAGE_SIZE
        if buffer.len() < PAGE_SIZE {
            buffer.resize(PAGE_SIZE, 0);
        }

        // Write to file
        self.file.write_all(&buffer)?;
        self.file.sync_all()?;

        Ok(())
    }

    /// Writes a page to disk without syncing (buffered write).
    ///
    /// This method writes the page to the OS buffer but does not guarantee
    /// durability until `sync()` is explicitly called. This enables
    /// Write-Ahead Logging to coordinate when data is flushed to disk.
    ///
    /// # Safety
    ///
    /// Data written with this method may be lost in a crash unless `sync()`
    /// is called. When using WAL, the WAL must be flushed first before
    /// calling `sync()` on the page file.
    ///
    /// # Arguments
    ///
    /// * `page` - The page to write
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Io`] if the write fails.
    pub fn write_page_buffered(&mut self, page: &Page) -> Result<(), PageError> {
        // Seek to the page offset
        let offset = page
            .id()
            .checked_mul(PAGE_SIZE as u64)
            .ok_or(PageError::PageTooLarge)?;
        self.file.seek(SeekFrom::Start(offset))?;

        // Prepare buffer with length prefix and data
        let mut buffer = Vec::with_capacity(PAGE_SIZE);

        // Write data length (8 bytes)
        let data_len = page.data().len() as u64;
        buffer.extend_from_slice(&data_len.to_le_bytes());

        // Write actual data
        buffer.extend_from_slice(page.data());

        // Ensure buffer doesn't exceed PAGE_SIZE (would corrupt page alignment)
        if buffer.len() > PAGE_SIZE {
            return Err(PageError::PageTooLarge);
        }

        // Pad to PAGE_SIZE
        if buffer.len() < PAGE_SIZE {
            buffer.resize(PAGE_SIZE, 0);
        }

        // Write to file WITHOUT sync (buffered)
        self.file.write_all(&buffer)?;

        Ok(())
    }

    /// Flushes all pending writes to disk.
    ///
    /// Ensures durability by calling `fsync` on the underlying file.
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Io`] if the sync fails.
    pub fn sync(&mut self) -> Result<(), PageError> {
        self.file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
