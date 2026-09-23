//! Host block-device shims for the storage HAL.
//!
//! This module is the host (std) side of the [`BlockDevice`] seam defined in
//! [`relvar_storage_core::device`]. It replaces the old `PageFile`: page I/O
//! now goes through the [`BlockDevice`] trait so the same heap/WAL code can
//! run against any medium (a file here, flash on an embedded target).
//!
//! [`BlockDevice`]: relvar_storage_core::device::BlockDevice
//!
//! # Durability
//!
//! [`FileBlockDevice::write_page`] only hands bytes to the OS; durability
//! comes from [`FileBlockDevice::flush`] (`fsync`), exactly as the
//! [`BlockDevice`] contract specifies. Callers decide when durability is
//! required — [`crate::storage::HeapFile`] flushes after every page write to
//! preserve the old `PageFile::write_page` contract.

use relvar_storage_core::device::BlockDevice;
use relvar_storage_core::page::{PAGE_SIZE, PageId};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// File-backed [`BlockDevice`]: the host implementation of the storage HAL.
///
/// Pages live at fixed offsets (`page_id * PAGE_SIZE`) in a regular file,
/// exactly as the old `PageFile` laid them out, so existing heap files open
/// unchanged. Reads of never-written pages return zeroed buffers per the
/// [`BlockDevice`] contract (sparse probing by the heap file is normal).
///
/// [`BlockDevice`]: relvar_storage_core::device::BlockDevice
pub struct FileBlockDevice {
    /// The underlying file handle.
    file: File,
}

impl FileBlockDevice {
    /// Creates a new device file, truncating any existing file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the device file will be created
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file cannot be created.
    pub fn create<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Ok(Self { file })
    }

    /// Opens an existing device file, or creates one if it doesn't exist.
    ///
    /// Unlike [`create`](Self::create), this preserves existing content.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the device file
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file cannot be opened.
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        Ok(Self { file })
    }

    /// Byte offset of a page within the file.
    fn page_offset(page_id: PageId) -> std::io::Result<u64> {
        page_id.checked_mul(PAGE_SIZE as u64).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "page id * PAGE_SIZE overflows u64",
            )
        })
    }
}

impl BlockDevice for FileBlockDevice {
    type Error = std::io::Error;

    fn read_page(&mut self, page_id: PageId, buf: &mut [u8; PAGE_SIZE]) -> Result<(), Self::Error> {
        let offset = Self::page_offset(page_id)?;
        self.file.seek(SeekFrom::Start(offset))?;

        // `read` may return short counts; loop to fill the buffer. A read of
        // 0 bytes means EOF — the page was never written — and per the HAL
        // contract those pages read back as zeros. A short tail (truncated
        // file) is zero-padded the same way.
        let mut filled = 0;
        while filled < PAGE_SIZE {
            let n = self.file.read(&mut buf[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        buf[filled..].fill(0);
        Ok(())
    }

    fn write_page(&mut self, page_id: PageId, buf: &[u8; PAGE_SIZE]) -> Result<(), Self::Error> {
        let offset = Self::page_offset(page_id)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(buf)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // OS durability lives here: fsync makes all preceding writes durable.
        self.file.sync_all()
    }

    fn page_count(&self) -> PageId {
        // The medium's size in pages. Writes are always whole pages, so the
        // length is page-aligned in practice; a short tail is not counted.
        self.file
            .metadata()
            .map(|metadata| metadata.len() / PAGE_SIZE as u64)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_storage_core::page::Page;
    use tempfile::NamedTempFile;

    /// Encodes a [`Page`] to its exact on-disk image for device I/O.
    fn page_bytes(page: &Page) -> [u8; PAGE_SIZE] {
        page.to_bytes().unwrap()
    }

    #[test]
    fn test_device_write_and_read_roundtrip() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Write a page
        {
            let mut device = FileBlockDevice::create(&path).unwrap();
            let page = Page::from_data(0, vec![1, 2, 3, 4, 5]).unwrap();
            device.write_page(0, &page_bytes(&page)).unwrap();
            device.flush().unwrap();
        }

        // Read it back
        {
            let mut device = FileBlockDevice::open(&path).unwrap();
            let mut buf = [0u8; PAGE_SIZE];
            device.read_page(0, &mut buf).unwrap();
            let page = Page::from_bytes(0, &buf).unwrap();

            assert_eq!(page.id(), 0);
            assert_eq!(page.data(), &[1, 2, 3, 4, 5]);
        }
    }

    #[test]
    fn test_device_multiple_pages() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Write multiple pages
        {
            let mut device = FileBlockDevice::create(&path).unwrap();

            for i in 0..5 {
                let page = Page::from_data(i, vec![i as u8; 10]).unwrap();
                device.write_page(i, &page_bytes(&page)).unwrap();
            }
            device.flush().unwrap();
        }

        // Read them back
        {
            let mut device = FileBlockDevice::open(&path).unwrap();

            for i in 0..5 {
                let mut buf = [0u8; PAGE_SIZE];
                device.read_page(i, &mut buf).unwrap();
                let page = Page::from_bytes(i, &buf).unwrap();
                assert_eq!(page.id(), i);
                assert_eq!(page.data().len(), 10);
                assert!(page.data().iter().all(|&b| b == i as u8));
            }
        }
    }

    #[test]
    fn test_device_empty_page() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let mut device = FileBlockDevice::create(&path).unwrap();

        // Write an empty page
        let page = Page::new(0);
        device.write_page(0, &page_bytes(&page)).unwrap();
        device.flush().unwrap();

        // Read it back
        let mut buf = [0u8; PAGE_SIZE];
        device.read_page(0, &mut buf).unwrap();
        let read_page = Page::from_bytes(0, &buf).unwrap();
        assert_eq!(read_page.id(), 0);
        assert!(read_page.is_empty());
    }

    #[test]
    fn test_device_overwrite() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let mut device = FileBlockDevice::create(&path).unwrap();

        // Write initial data
        let page1 = Page::from_data(0, vec![1, 2, 3]).unwrap();
        device.write_page(0, &page_bytes(&page1)).unwrap();

        // Overwrite with new data
        let page2 = Page::from_data(0, vec![4, 5, 6, 7]).unwrap();
        device.write_page(0, &page_bytes(&page2)).unwrap();
        device.flush().unwrap();

        // Read it back
        let mut buf = [0u8; PAGE_SIZE];
        device.read_page(0, &mut buf).unwrap();
        let read_page = Page::from_bytes(0, &buf).unwrap();
        assert_eq!(read_page.data(), &[4, 5, 6, 7]);
    }

    #[test]
    fn test_device_unwritten_page_reads_as_zeros() {
        // HAL contract: reads of never-written pages return zeroed buffers,
        // not errors. The heap file probes pages before they exist.
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let mut device = FileBlockDevice::create(&path).unwrap();

        // Nothing written yet: reads are zeroed, not errors.
        let mut out = [0xFFu8; PAGE_SIZE];
        device.read_page(0, &mut out).unwrap();
        assert_eq!(out, [0u8; PAGE_SIZE]);

        // Sparse write: the gap pages read back as zeros.
        let page = Page::from_data(5, vec![0xCD; 10]).unwrap();
        device.write_page(5, &page_bytes(&page)).unwrap();
        assert_eq!(device.page_count(), 6);

        for page_id in 0..5 {
            let mut gap = [0xFFu8; PAGE_SIZE];
            device.read_page(page_id, &mut gap).unwrap();
            assert_eq!(gap, [0u8; PAGE_SIZE], "gap page {page_id} not zeroed");
        }

        let mut written = [0u8; PAGE_SIZE];
        device.read_page(5, &mut written).unwrap();
        let read_page = Page::from_bytes(5, &written).unwrap();
        assert_eq!(read_page.data(), &[0xCD; 10]);
    }

    #[test]
    fn test_device_create_truncates() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let mut device = FileBlockDevice::create(&path).unwrap();
            let page = Page::from_data(0, vec![1, 2, 3]).unwrap();
            device.write_page(0, &page_bytes(&page)).unwrap();
            device.flush().unwrap();
            assert_eq!(device.page_count(), 1);
        }

        // Re-creating truncates the file.
        {
            let device = FileBlockDevice::create(&path).unwrap();
            assert_eq!(device.page_count(), 0);
        }

        // Opening preserves content.
        {
            let mut device = FileBlockDevice::create(&path).unwrap();
            let page = Page::from_data(0, vec![9, 9, 9]).unwrap();
            device.write_page(0, &page_bytes(&page)).unwrap();
            device.flush().unwrap();
        }
        {
            let mut device = FileBlockDevice::open(&path).unwrap();
            assert_eq!(device.page_count(), 1);
            let mut buf = [0u8; PAGE_SIZE];
            device.read_page(0, &mut buf).unwrap();
            let read_page = Page::from_bytes(0, &buf).unwrap();
            assert_eq!(read_page.data(), &[9, 9, 9]);
        }
    }

    #[test]
    fn test_device_flush_persists_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let mut device = FileBlockDevice::create(&path).unwrap();
            let page = Page::from_data(0, vec![42, 43, 44]).unwrap();
            device.write_page(0, &page_bytes(&page)).unwrap();
            device.flush().unwrap();
        }

        // Data survives reopen after flush.
        {
            let mut device = FileBlockDevice::open(&path).unwrap();
            let mut buf = [0u8; PAGE_SIZE];
            device.read_page(0, &mut buf).unwrap();
            let read_page = Page::from_bytes(0, &buf).unwrap();
            assert_eq!(read_page.data(), &[42, 43, 44]);
        }
    }

    #[test]
    fn test_device_page_count_tracks_writes() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let mut device = FileBlockDevice::create(&path).unwrap();
        assert_eq!(device.page_count(), 0);

        let page = Page::from_data(0, vec![1]).unwrap();
        device.write_page(0, &page_bytes(&page)).unwrap();
        assert_eq!(device.page_count(), 1);

        // Sparse write grows the medium: highest written page id + 1.
        let page = Page::from_data(3, vec![2]).unwrap();
        device.write_page(3, &page_bytes(&page)).unwrap();
        assert_eq!(device.page_count(), 4);
    }

    #[test]
    fn test_device_raw_bytes_roundtrip() {
        // The device is byte-transparent: it never parses page contents.
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let mut device = FileBlockDevice::create(&path).unwrap();
        let raw = [0xABu8; PAGE_SIZE];
        device.write_page(2, &raw).unwrap();
        device.flush().unwrap();

        let mut out = [0u8; PAGE_SIZE];
        device.read_page(2, &mut out).unwrap();
        assert_eq!(out, raw);
    }
}
