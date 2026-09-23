//! The `BlockDevice` hardware abstraction layer.
//!
//! This module is the single seam between Relvar's on-disk formats and the
//! hardware they live on. Everything above this seam ([`page`](crate::page),
//! [`slotted`](crate::slotted), [`wal`](crate::wal)) speaks only in
//! page-sized buffers; everything below it is one trait:
//! [`BlockDevice`].
//!
//! * On a **host**, `relvar-storage` implements `BlockDevice` over a file
//!   (the rewire worker's job): `read_page`/`write_page` seek to
//!   `page_id * PAGE_SIZE`, `flush` fsyncs, `page_count` derives from the
//!   file length.
//! * On an **embedded** target, the board support implements `BlockDevice`
//!   over flash (or FRAM/MRAM): erase-before-write, wear-levelling, and
//!   bad-block handling all live behind this trait, invisible to the
//!   format code.
//!
//! [`MemBlockDevice`] is an in-memory implementation for tests and for
//! embedded bring-up before the flash driver exists.

use crate::page::{PAGE_SIZE, PageId};
use alloc::vec::Vec;

/// Raw page-sized block access to the storage medium.
///
/// The HAL contract:
/// * All I/O is in whole [`PAGE_SIZE`]-byte pages; there are no partial
///   page reads or writes.
/// * [`read_page`](Self::read_page) of a never-written page returns a
///   zeroed buffer (rather than an error). Sparse address spaces are
///   normal — the heap file probes pages before they exist.
/// * [`write_page`](Self::write_page) to any `page_id` succeeds, growing
///   the medium as needed; unwritten pages in between read back as zeros.
/// * [`flush`](Self::flush) makes all preceding writes durable. Callers
///   (WAL, checkpoints) decide when durability is required; the device
///   decides how to provide it.
/// * `page_id` values are expected to fit in `usize` on the target, as
///   with the `as` casts used throughout the storage layer.
pub trait BlockDevice {
    /// The device's error type (e.g. `std::io::Error` for a file-backed
    /// device, [`core::convert::Infallible`] for RAM).
    type Error: core::fmt::Debug;

    /// Reads one page into `buf`.
    ///
    /// Reads of unwritten pages return zeroed buffers — see the trait-level
    /// contract.
    fn read_page(&mut self, page_id: PageId, buf: &mut [u8; PAGE_SIZE]) -> Result<(), Self::Error>;

    /// Writes one page from `buf`, growing the medium if needed.
    fn write_page(&mut self, page_id: PageId, buf: &[u8; PAGE_SIZE]) -> Result<(), Self::Error>;

    /// Makes all preceding writes durable.
    fn flush(&mut self) -> Result<(), Self::Error>;

    /// Number of pages currently addressable (highest written page id + 1,
    /// or the medium's size in pages for fixed-size media).
    fn page_count(&self) -> PageId;
}

/// In-memory [`BlockDevice`]: a `Vec` of pages.
///
/// Always available, including in `no_std` builds. Used for unit tests
/// and for embedded bring-up before a flash driver exists. Cannot fail:
/// its error type is [`core::convert::Infallible`].
#[derive(Debug, Clone, Default)]
pub struct MemBlockDevice {
    pages: Vec<[u8; PAGE_SIZE]>,
}

impl MemBlockDevice {
    /// Creates an empty in-memory device holding zero pages.
    pub fn new() -> Self {
        Self { pages: Vec::new() }
    }
}

impl BlockDevice for MemBlockDevice {
    type Error = core::convert::Infallible;

    fn read_page(&mut self, page_id: PageId, buf: &mut [u8; PAGE_SIZE]) -> Result<(), Self::Error> {
        match self.pages.get(page_id as usize) {
            Some(page) => buf.copy_from_slice(page),
            // Contract: unwritten pages read as zeros.
            None => buf.fill(0),
        }
        Ok(())
    }

    fn write_page(&mut self, page_id: PageId, buf: &[u8; PAGE_SIZE]) -> Result<(), Self::Error> {
        let index = page_id as usize;
        if index >= self.pages.len() {
            self.pages.resize(index + 1, [0u8; PAGE_SIZE]);
        }
        self.pages[index].copy_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // RAM is always durable.
        Ok(())
    }

    fn page_count(&self) -> PageId {
        self.pages.len() as PageId
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(byte: u8) -> [u8; PAGE_SIZE] {
        [byte; PAGE_SIZE]
    }

    #[test]
    fn test_new_device_is_empty() {
        let device = MemBlockDevice::new();
        assert_eq!(device.page_count(), 0);
    }

    #[test]
    fn test_default_matches_new() {
        let device = MemBlockDevice::default();
        assert_eq!(device.page_count(), 0);
    }

    #[test]
    fn test_write_then_read_roundtrip() {
        let mut device = MemBlockDevice::new();
        let buf = pattern(0xAB);

        device.write_page(0, &buf).unwrap();
        assert_eq!(device.page_count(), 1);

        let mut out = [0u8; PAGE_SIZE];
        device.read_page(0, &mut out).unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn test_read_unwritten_page_returns_zeros() {
        let mut device = MemBlockDevice::new();

        // Nothing written yet: reads are zeroed, not errors.
        let mut out = [0xFFu8; PAGE_SIZE];
        device.read_page(0, &mut out).unwrap();
        assert_eq!(out, [0u8; PAGE_SIZE]);

        // Sparse write: the gap pages read back as zeros.
        device.write_page(5, &pattern(0xCD)).unwrap();
        assert_eq!(device.page_count(), 6);

        for page_id in 0..5 {
            let mut gap = [0xFFu8; PAGE_SIZE];
            device.read_page(page_id, &mut gap).unwrap();
            assert_eq!(gap, [0u8; PAGE_SIZE], "gap page {page_id} not zeroed");
        }

        let mut written = [0u8; PAGE_SIZE];
        device.read_page(5, &mut written).unwrap();
        assert_eq!(written, pattern(0xCD));
    }

    #[test]
    fn test_overwrite_page() {
        let mut device = MemBlockDevice::new();
        device.write_page(2, &pattern(0x11)).unwrap();
        device.write_page(2, &pattern(0x22)).unwrap();

        assert_eq!(device.page_count(), 3);

        let mut out = [0u8; PAGE_SIZE];
        device.read_page(2, &mut out).unwrap();
        assert_eq!(out, pattern(0x22));
    }

    #[test]
    fn test_flush_succeeds() {
        let mut device = MemBlockDevice::new();
        device.write_page(0, &pattern(0x99)).unwrap();
        device.flush().unwrap();

        let mut out = [0u8; PAGE_SIZE];
        device.read_page(0, &mut out).unwrap();
        assert_eq!(out, pattern(0x99));
    }

    #[test]
    fn test_device_is_object_safe_friendly() {
        // The trait is usable behind a generic bound, as the storage engine will.
        fn page_count_of<D: BlockDevice>(device: &D) -> PageId {
            device.page_count()
        }
        let device = MemBlockDevice::new();
        assert_eq!(page_count_of(&device), 0);
    }
}
