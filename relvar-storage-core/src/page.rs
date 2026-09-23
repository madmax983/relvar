//! Page-based I/O for fixed-size disk blocks.
//!
//! This module provides the lowest-level storage abstraction: fixed-size pages
//! that serve as the unit of I/O between disk and memory. All higher-level
//! storage structures (heap files, indexes) are built on top of pages.
//!
//! Moved verbatim from `relvar-storage`'s page module into the `no_std`
//! storage core. The file-backed [`PageFile`](https://github.com/madmax983/relvar)
//! stays on the host side; what moved here is the [`Page`] type itself plus
//! the exact on-disk framing (8-byte little-endian length header, zero-padded
//! to [`PAGE_SIZE`]), so every target encodes and decodes identical bytes.
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
//! ```
//! use relvar_storage_core::page::{Page, PAGE_SIZE};
//!
//! // Create a page and encode it to its exact on-disk image.
//! let page = Page::from_data(0, vec![1, 2, 3, 4, 5]).unwrap();
//! let bytes = page.to_bytes().unwrap();
//! assert_eq!(bytes.len(), PAGE_SIZE);
//! assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 5);
//!
//! // Decode it back. The page id is positional (offset = id * PAGE_SIZE),
//! // so the caller supplies it — it is not stored in the bytes.
//! let decoded = Page::from_bytes(0, &bytes).unwrap();
//! assert_eq!(decoded, page);
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
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
///
/// Core-only: unlike the host-side page error, there is no `std::io::Error`
/// variant here — I/O failures belong to the [`crate::device::BlockDevice`]
/// implementation on the host side.
#[derive(Debug, Error)]
pub enum PageError {
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
/// use relvar_storage_core::page::{Page, PAGE_SIZE};
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
    /// # Examples
    ///
    /// ```
    /// use relvar_storage_core::page::Page;
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
    /// # Errors
    ///
    /// Returns [`PageError::PageTooLarge`] if `data.len() > PAGE_SIZE - 8`.
    /// The 8 bytes are reserved for the length prefix written by
    /// [`to_bytes`](Self::to_bytes), keeping the total on-disk size at
    /// exactly [`PAGE_SIZE`].
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage_core::page::Page;
    ///
    /// let page = Page::from_data(42, vec![1, 2, 3]).unwrap();
    /// assert_eq!(page.id(), 42);
    /// assert_eq!(page.data(), &[1, 2, 3]);
    /// ```
    pub fn from_data(id: PageId, data: Vec<u8>) -> Result<Self, PageError> {
        // Limit to PAGE_SIZE - 8 to account for the 8-byte length prefix
        // written by to_bytes(), ensuring total on-disk size is exactly PAGE_SIZE
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
    /// use relvar_storage_core::page::Page;
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
    /// This is `PAGE_SIZE - data.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage_core::page::{Page, PAGE_SIZE};
    ///
    /// let page = Page::new(1);
    /// // A new page starts completely empty (data length is 0).
    /// assert_eq!(page.available_space(), PAGE_SIZE);
    /// ```
    pub fn available_space(&self) -> usize {
        PAGE_SIZE - self.data.len()
    }

    /// Returns `true` if the page contains no data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Encodes this page to its exact on-disk image: `[u8; PAGE_SIZE]`.
    ///
    /// Layout: 8-byte little-endian data length, then the data, then zero
    /// padding to fill [`PAGE_SIZE`]. This is the same framing the host-side
    /// page file writes; it moved here so every target produces identical
    /// bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PageError::PageTooLarge`] if the length-prefixed image would
    /// exceed [`PAGE_SIZE`] (possible when data was installed via
    /// [`set_data`](Self::set_data), which permits up to `PAGE_SIZE` bytes).
    pub fn to_bytes(&self) -> Result<[u8; PAGE_SIZE], PageError> {
        // Mirror of the host PageFile::write_page buffer construction.
        let mut buffer = Vec::with_capacity(PAGE_SIZE);

        // Write data length (8 bytes)
        let data_len = self.data.len() as u64;
        buffer.extend_from_slice(&data_len.to_le_bytes());

        // Write actual data
        buffer.extend_from_slice(&self.data);

        // Ensure buffer doesn't exceed PAGE_SIZE (would corrupt page alignment)
        if buffer.len() > PAGE_SIZE {
            return Err(PageError::PageTooLarge);
        }

        // Pad to PAGE_SIZE
        buffer.resize(PAGE_SIZE, 0);

        let mut out = [0u8; PAGE_SIZE];
        out.copy_from_slice(&buffer);
        Ok(out)
    }

    /// Decodes a page from its exact on-disk image.
    ///
    /// The page ID is positional (`offset = page_id * PAGE_SIZE`) and is not
    /// stored in the bytes, so the caller — which knows which page it read —
    /// supplies it.
    ///
    /// # Errors
    ///
    /// Returns [`PageError::Serialization`] if the length prefix is corrupt
    /// (declares more data than [`PAGE_SIZE`] - 8 allows, or more than the
    /// buffer holds).
    pub fn from_bytes(id: PageId, bytes: &[u8; PAGE_SIZE]) -> Result<Self, PageError> {
        // Mirror of the host PageFile::parse_page_data.
        let buffer: &[u8] = bytes;

        // We need at least 8 bytes for the length prefix
        if buffer.len() < 8 {
            return Err(PageError::Serialization(String::from("Page too short")));
        }

        let data_len_u64 = u64::from_le_bytes(buffer[0..8].try_into().map_err(|_| {
            PageError::Serialization(String::from("Failed to parse length prefix"))
        })?);

        // Check if data length exceeds PAGE_SIZE - 8 (maximum possible data)
        if data_len_u64 > (PAGE_SIZE - 8) as u64 {
            return Err(PageError::Serialization(alloc::format!(
                "Page data length {} exceeds maximum {}",
                data_len_u64,
                PAGE_SIZE - 8
            )));
        }

        let data_len = data_len_u64 as usize;
        let required_len = data_len + 8; // No overflow possible (checked above)

        if required_len > buffer.len() {
            return Err(PageError::Serialization(alloc::format!(
                "Corrupted page: length prefix says {}, but only {} bytes available",
                data_len,
                buffer.len()
            )));
        }

        let actual_data = buffer[8..8 + data_len].to_vec();
        Page::from_data(id, actual_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_page_creation() {
        let page = Page::new(0);
        assert_eq!(page.id(), 0);
        assert!(page.is_empty());
        assert_eq!(page.available_space(), PAGE_SIZE);
    }

    #[test]
    fn test_page_from_data() {
        let data = vec![1, 2, 3, 4, 5];
        let page = Page::from_data(42, data.clone()).unwrap();

        assert_eq!(page.id(), 42);
        assert_eq!(page.data(), &data);
        assert_eq!(page.available_space(), PAGE_SIZE - 5);
    }

    #[test]
    fn test_page_too_large() {
        let data = vec![0u8; PAGE_SIZE + 1];
        let result = Page::from_data(0, data);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PageError::PageTooLarge));
    }

    #[test]
    fn test_page_set_data() {
        let mut page = Page::new(0);
        let data = vec![1, 2, 3];

        page.set_data(data.clone()).unwrap();
        assert_eq!(page.data(), &data);
    }

    #[test]
    fn test_page_set_data_too_large() {
        let mut page = Page::new(0);
        let huge_data = vec![0; PAGE_SIZE + 1];

        let result = page.set_data(huge_data);
        assert!(matches!(result, Err(PageError::PageTooLarge)));
    }

    #[test]
    fn test_header_roundtrip() {
        let data = vec![1, 2, 3, 4, 5];
        let page = Page::from_data(7, data.clone()).unwrap();

        let bytes = page.to_bytes().unwrap();
        assert_eq!(bytes.len(), PAGE_SIZE);

        // 8-byte little-endian length header
        assert_eq!(
            u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            data.len() as u64
        );
        // Payload follows the header
        assert_eq!(&bytes[8..8 + data.len()], data.as_slice());
        // Zero padding fills the rest
        assert!(bytes[8 + data.len()..].iter().all(|&b| b == 0));

        // Decode roundtrip preserves id and data
        let decoded = Page::from_bytes(7, &bytes).unwrap();
        assert_eq!(decoded, page);
    }

    #[test]
    fn test_header_roundtrip_empty_page() {
        let page = Page::new(3);
        let bytes = page.to_bytes().unwrap();

        assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 0);
        assert!(bytes[8..].iter().all(|&b| b == 0));

        let decoded = Page::from_bytes(3, &bytes).unwrap();
        assert_eq!(decoded, page);
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_header_roundtrip_max_data() {
        let data = vec![0xABu8; PAGE_SIZE - 8];
        let page = Page::from_data(9, data.clone()).unwrap();

        let bytes = page.to_bytes().unwrap();
        assert_eq!(
            u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            (PAGE_SIZE - 8) as u64
        );

        let decoded = Page::from_bytes(9, &bytes).unwrap();
        assert_eq!(decoded.data(), data.as_slice());
    }

    #[test]
    fn test_to_bytes_rejects_oversize_data() {
        // set_data permits up to PAGE_SIZE bytes, but the 8-byte length
        // prefix would then overflow the fixed-size image.
        let mut page = Page::new(0);
        page.set_data(vec![0u8; PAGE_SIZE - 4]).unwrap();

        let result = page.to_bytes();
        assert!(matches!(result, Err(PageError::PageTooLarge)));
    }

    #[test]
    fn test_from_bytes_rejects_corrupt_length() {
        // Length prefix larger than PAGE_SIZE - 8.
        let mut bytes = [0u8; PAGE_SIZE];
        bytes[0..8].copy_from_slice(&9999u64.to_le_bytes());

        let result = Page::from_bytes(0, &bytes);
        assert!(result.is_err());
        match result {
            Err(PageError::Serialization(msg)) => {
                assert!(msg.contains("exceeds maximum"));
            }
            _ => panic!("Expected Serialization error"),
        }
    }

    #[test]
    fn test_from_bytes_rejects_u64_max_length() {
        let mut bytes = [0u8; PAGE_SIZE];
        bytes[0..8].copy_from_slice(&u64::MAX.to_le_bytes());

        let result = Page::from_bytes(0, &bytes);
        assert!(matches!(result, Err(PageError::Serialization(_))));
    }

    #[test]
    fn test_from_bytes_length_boundary() {
        // Max allowed length (PAGE_SIZE - 8) decodes fine.
        let mut bytes = [0u8; PAGE_SIZE];
        bytes[0..8].copy_from_slice(&((PAGE_SIZE - 8) as u64).to_le_bytes());
        let page = Page::from_bytes(0, &bytes).unwrap();
        assert_eq!(page.data().len(), PAGE_SIZE - 8);

        // Max allowed length + 1 is rejected.
        let mut bytes = [0u8; PAGE_SIZE];
        bytes[0..8].copy_from_slice(&((PAGE_SIZE - 7) as u64).to_le_bytes());
        let result = Page::from_bytes(0, &bytes);
        assert!(matches!(result, Err(PageError::Serialization(_))));
    }

    #[test]
    fn test_page_id_is_positional_not_stored() {
        // The on-disk image carries no page id; the same bytes decode under
        // any id the caller supplies.
        let page = Page::from_data(11, vec![1, 2, 3]).unwrap();
        let bytes = page.to_bytes().unwrap();

        let as_other = Page::from_bytes(99, &bytes).unwrap();
        assert_eq!(as_other.id(), 99);
        assert_eq!(as_other.data(), page.data());
    }
}
