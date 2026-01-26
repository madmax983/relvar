use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

/// Page size in bytes (4KB is a common choice)
pub const PAGE_SIZE: usize = 4096;

/// Page ID type
pub type PageId = u64;

#[derive(Debug, Error)]
pub enum PageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Page data exceeds maximum size")]
    PageTooLarge,
}

/// A page is a fixed-size block of data stored on disk.
/// Pages are the unit of I/O between disk and memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// Page identifier
    id: PageId,
    /// Raw page data (up to PAGE_SIZE bytes)
    data: Vec<u8>,
}

impl Page {
    /// Create a new empty page
    pub fn new(id: PageId) -> Self {
        Self {
            id,
            data: Vec::new(),
        }
    }

    /// Create a page from data
    pub fn from_data(id: PageId, data: Vec<u8>) -> Result<Self, PageError> {
        if data.len() > PAGE_SIZE {
            return Err(PageError::PageTooLarge);
        }
        Ok(Self { id, data })
    }

    /// Get page ID
    pub fn id(&self) -> PageId {
        self.id
    }

    /// Get page data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Set page data
    pub fn set_data(&mut self, data: Vec<u8>) -> Result<(), PageError> {
        if data.len() > PAGE_SIZE {
            return Err(PageError::PageTooLarge);
        }
        self.data = data;
        Ok(())
    }

    /// Get available space in the page
    pub fn available_space(&self) -> usize {
        PAGE_SIZE - self.data.len()
    }

    /// Check if the page is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Page file manages pages on disk
pub struct PageFile {
    file: File,
}

impl PageFile {
    /// Create a new page file
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, PageError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Ok(Self { file })
    }

    /// Open an existing page file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, PageError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        Ok(Self { file })
    }

    /// Read a page from disk
    pub fn read_page(&mut self, page_id: PageId) -> Result<Page, PageError> {
        // Seek to the page offset
        let offset = page_id * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;

        // Read the page data
        let mut buffer = vec![0u8; PAGE_SIZE];
        let bytes_read = self.file.read(&mut buffer)?;

        // Truncate to actual bytes read
        buffer.truncate(bytes_read);

        // Find the end of actual data (before padding)
        // We store the actual data length in the first 8 bytes
        if buffer.len() >= 8 {
            let data_len = u64::from_le_bytes(buffer[0..8].try_into().unwrap()) as usize;
            if data_len + 8 <= buffer.len() {
                let actual_data = buffer[8..8 + data_len].to_vec();
                return Page::from_data(page_id, actual_data);
            }
        }

        // If we can't read the length, assume empty page
        Ok(Page::new(page_id))
    }

    /// Write a page to disk
    pub fn write_page(&mut self, page: &Page) -> Result<(), PageError> {
        // Seek to the page offset
        let offset = page.id() * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;

        // Prepare buffer with length prefix and data
        let mut buffer = Vec::with_capacity(PAGE_SIZE);

        // Write data length (8 bytes)
        let data_len = page.data().len() as u64;
        buffer.extend_from_slice(&data_len.to_le_bytes());

        // Write actual data
        buffer.extend_from_slice(page.data());

        // Pad to PAGE_SIZE
        if buffer.len() < PAGE_SIZE {
            buffer.resize(PAGE_SIZE, 0);
        }

        // Write to file
        self.file.write_all(&buffer)?;
        self.file.sync_all()?;

        Ok(())
    }

    /// Flush all changes to disk
    pub fn sync(&mut self) -> Result<(), PageError> {
        self.file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

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
    fn test_page_file_write_and_read() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write a page
        {
            let mut page_file = PageFile::create(path).unwrap();
            let data = vec![1, 2, 3, 4, 5];
            let page = Page::from_data(0, data.clone()).unwrap();

            page_file.write_page(&page).unwrap();
        }

        // Read it back
        {
            let mut page_file = PageFile::open(path).unwrap();
            let page = page_file.read_page(0).unwrap();

            assert_eq!(page.id(), 0);
            assert_eq!(page.data(), &[1, 2, 3, 4, 5]);
        }
    }

    #[test]
    fn test_page_file_multiple_pages() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write multiple pages
        {
            let mut page_file = PageFile::create(path).unwrap();

            for i in 0..5 {
                let data = vec![i as u8; 10];
                let page = Page::from_data(i, data).unwrap();
                page_file.write_page(&page).unwrap();
            }
        }

        // Read them back
        {
            let mut page_file = PageFile::open(path).unwrap();

            for i in 0..5 {
                let page = page_file.read_page(i).unwrap();
                assert_eq!(page.id(), i);
                assert_eq!(page.data().len(), 10);
                assert!(page.data().iter().all(|&b| b == i as u8));
            }
        }
    }

    #[test]
    fn test_page_file_empty_page() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut page_file = PageFile::create(path).unwrap();

        // Write an empty page
        let page = Page::new(0);
        page_file.write_page(&page).unwrap();

        // Read it back
        let read_page = page_file.read_page(0).unwrap();
        assert_eq!(read_page.id(), 0);
        assert!(read_page.is_empty());
    }

    #[test]
    fn test_page_file_overwrite() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut page_file = PageFile::create(path).unwrap();

        // Write initial data
        let page1 = Page::from_data(0, vec![1, 2, 3]).unwrap();
        page_file.write_page(&page1).unwrap();

        // Overwrite with new data
        let page2 = Page::from_data(0, vec![4, 5, 6, 7]).unwrap();
        page_file.write_page(&page2).unwrap();

        // Read it back
        let read_page = page_file.read_page(0).unwrap();
        assert_eq!(read_page.data(), &[4, 5, 6, 7]);
    }
}
