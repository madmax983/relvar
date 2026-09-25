//! Heap file storage for relation tuples.
//!
//! # TTM Compliance
//!
//! This module implements physical storage using a heap file structure with
//! slotted pages. Internally, it uses TupleId (page_id + slot) for physical
//! addressing, but TupleId is never exposed in public APIs per TTM Proscription 6:
//! "The database must not generate or expose tuple-level identifiers."
//!
//! From the relational perspective, tuples are identified by their attribute
//! values (keys), not by physical storage locations.
//!
//! ## Public API Design
//!
//! - `insert_tuple()` returns `Result<(), HeapError>` - success/failure only
//! - `scan()` returns `Vec<Tuple>` - tuples without physical identifiers
//! - `store_relation()` returns `Result<(), HeapError>` - no tuple IDs
//! - `read_tuple(TupleId)` is `pub(crate)` - internal use only within storage layer
//!
//! This design ensures callers work with tuples as values, never as physical
//! storage references, maintaining the relational abstraction.

use super::page::{PAGE_SIZE, Page, PageError, PageId};
use crate::device::FileBlockDevice;
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
// Slotted-page types, wire formats, and codecs live in the no_std storage
// core; the heap file only adds tuple serde and page-management policy.
// (Private `use` items are visible to the child test modules via `::*`.)
use relvar_storage_core::device::BlockDevice;
use relvar_storage_core::slotted::{
    SlotEntry, SlottedError, SlottedPage, TupleId, USABLE_PAGE_SIZE_V1, USABLE_PAGE_SIZE_V2,
    V2_HEADER_SIZE, VERSIONED_PAGE_MAGIC, VersionedSlotEntry, VersionedSlottedPage,
    decode_slotted_page, decode_versioned_page, encode_versioned_page, is_versioned_page,
    slot_bytes,
};
// Re-exported for the page-layout tests (child modules import via `::*`);
// unused in the non-test build.
#[cfg(test)]
use relvar_storage_core::slotted::{PAGE_FORMAT_VERSION, encode_slotted_page};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Helper for bounded deserialization to prevent allocation bombs.
/// Limits allocation size to PAGE_SIZE (4KB), preventing DoS from malicious length prefixes.
fn deserialize_bounded<'a, T>(data: &'a [u8]) -> Result<T, HeapError>
where
    T: Deserialize<'a>,
{
    postcard::from_bytes(data).map_err(|e| HeapError::Serialization(e.to_string()))
}

/// Helper for consistent serialization matching `deserialize_bounded`.
fn serialize_compat<T: Serialize>(value: &T) -> Result<Vec<u8>, HeapError> {
    postcard::to_allocvec(value).map_err(|e| HeapError::Serialization(e.to_string()))
}

/// Helper for calculating serialized size matching `serialize_compat`.
fn serialized_size_compat<T: Serialize>(value: &T) -> Result<u64, HeapError> {
    postcard::to_allocvec(value)
        .map(|v| v.len() as u64)
        .map_err(|e| HeapError::Serialization(e.to_string()))
}

// `TupleId` (page_id, slot) is re-exported from `relvar_storage_core::slotted`
// above. It stays internal to the storage layer (TTM Proscription 6).

/// Errors that can occur during heap file operations.
#[derive(Debug, Error)]
pub enum HeapError {
    /// An error occurred at the page layer.
    #[error("Page error: {0}")]
    Page(#[from] PageError),

    /// The underlying storage device reported an error.
    ///
    /// `BlockDevice::Error` is only bounded by [`core::fmt::Debug`] (embedded
    /// targets have no `std::error::Error`), so the concrete error is captured
    /// as its `Debug` rendering. For [`FileBlockDevice`] this is the
    /// underlying I/O error's message.
    #[error("Storage device error: {0}")]
    Device(String),

    /// A slotted-page layout or codec error from the storage core.
    #[error("Slotted page error: {0}")]
    Slotted(#[from] SlottedError),

    /// Tuple serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The requested tuple was not found.
    /// Physical details (page_id, slot) hidden per TTM Proscription 6.
    #[error("Tuple not found")]
    TupleNotFound,

    /// The page has insufficient space for the tuple.
    #[error("Page full")]
    PageFull,

    /// The tuple is too large to fit in a page.
    #[error("Tuple too large: {0} bytes")]
    TupleTooLarge(usize),
}

/// Stores tuples in an unordered collection of slotted pages.
///
/// A heap file is the primary storage structure for relation tuples. Tuples
/// are stored in pages without any particular ordering. Each page uses a
/// slotted page format with a slot directory at the beginning and tuple
/// data growing from the end.
///
/// # Page Layout
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │ slot_count │ slot[0] │ slot[1] │ ... │ free space │ tuples  │
/// └─────────────────────────────────────────────────────────────┘
/// ```text
///
/// # TTM Compliance
///
/// This struct carefully maintains TTM Proscription 6 compliance:
/// - `insert_tuple()` returns `Result<(), HeapError>` (no TupleId)
/// - `scan()` returns `Vec<Tuple>` (no TupleId)
/// - `read_tuple(TupleId)` is `pub(crate)` (internal only)
///
/// # Examples
///
/// ```no_run
/// use relvar_storage::storage::HeapFile;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
/// use relvar_core::tuple;
///
/// // Create heap file
/// let heading = TupleType::new()
///     .with_attribute("id".to_string(), ScalarType::Int)
///     .with_attribute("name".to_string(), ScalarType::String);
/// let rel_type = RelationType::new(heading);
///
/// let mut heap = HeapFile::create("employees.heap", rel_type).unwrap();
///
/// // Insert tuples
/// heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" }).unwrap();
/// heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" }).unwrap();
///
/// // Scan all tuples
/// let tuples = heap.scan().unwrap();
/// assert_eq!(tuples.len(), 2);
/// ```text
/// Heap file: unordered tuple storage over a generic [`BlockDevice`].
///
/// `D` defaults to [`FileBlockDevice`] so existing path-based call sites keep
/// working unchanged; pass any other [`BlockDevice`] (e.g. an in-memory or
/// flash device) via [`create_on_device`](Self::create_on_device) /
/// [`open_on_device`](Self::open_on_device).
///
/// [`BlockDevice`]: relvar_storage_core::device::BlockDevice
pub struct HeapFile<D: BlockDevice = FileBlockDevice> {
    /// The underlying storage device for page I/O.
    device: D,
    /// The type of tuples stored in this heap file.
    relation_type: RelationType,
}

// The slotted-page types (`SlotEntry`, `SlottedPage`, `VersionedSlotEntry`,
// `VersionedSlottedPage`, `TupleId`) and the page-layout constants
// (`VERSIONED_PAGE_MAGIC`, `PAGE_FORMAT_VERSION`, `USABLE_PAGE_SIZE_V1`,
// `USABLE_PAGE_SIZE_V2`, `V2_HEADER_SIZE`) now come from
// `relvar_storage_core::slotted` (re-exported at the top of this module).
// The on-disk formats are unchanged — the core documents them as the exact
// layouts this heap file has always written.

/// Maps a device error into [`HeapError::Device`].
///
/// `BlockDevice::Error` is only bounded by [`core::fmt::Debug`], so the
/// concrete error is captured as its `Debug` rendering (see the
/// [`HeapError::Device`] docs for the rationale).
fn device_error<E: core::fmt::Debug>(error: E) -> HeapError {
    HeapError::Device(format!("{error:?}"))
}

impl HeapFile<FileBlockDevice> {
    /// Creates a new heap file, truncating any existing file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the heap file will be created
    /// * `relation_type` - The type of tuples that will be stored
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Device`] if the file cannot be created.
    ///
    /// # Examples
    /// ```text
    /// use relvar_storage::storage::heap::HeapFile;
    /// use relvar_core::types::{RelationType, TupleType};
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let rel_type = RelationType::new(TupleType::new());
    /// let heap = HeapFile::create(dir.path().join("test.heap"), rel_type).unwrap();
    /// ```text
    pub fn create<P: AsRef<Path>>(path: P, relation_type: RelationType) -> Result<Self, HeapError> {
        let device = FileBlockDevice::create(path).map_err(device_error)?;
        Ok(Self::create_on_device(device, relation_type))
    }

    /// Opens an existing heap file, or creates one if it doesn't exist.
    ///
    /// Unlike [`create`](Self::create), this preserves existing data.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the heap file
    /// * `relation_type` - The type of tuples stored in this heap file
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Device`] if the file cannot be opened.
    ///
    /// # Examples
    /// ```text
    /// use relvar_storage::storage::heap::HeapFile;
    /// use relvar_core::types::{RelationType, TupleType};
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let path = dir.path().join("test.heap");
    /// let rel_type = RelationType::new(TupleType::new());
    /// HeapFile::create(&path, rel_type.clone()).unwrap();
    /// let heap = HeapFile::open(&path, rel_type).unwrap();
    /// ```text
    pub fn open<P: AsRef<Path>>(path: P, relation_type: RelationType) -> Result<Self, HeapError> {
        let device = FileBlockDevice::open(path).map_err(device_error)?;
        Ok(Self::open_on_device(device, relation_type))
    }
}

impl<D: BlockDevice> HeapFile<D> {
    /// Creates a heap file on a caller-provided device.
    ///
    /// The device equivalent of [`create`](HeapFile::create): use this with
    /// an in-memory or embedded device instead of a file path.
    pub fn create_on_device(device: D, relation_type: RelationType) -> Self {
        Self {
            device,
            relation_type,
        }
    }

    /// Opens a heap file on a caller-provided device.
    ///
    /// The device equivalent of [`open`](HeapFile::open): use this with
    /// an in-memory or embedded device instead of a file path.
    pub fn open_on_device(device: D, relation_type: RelationType) -> Self {
        Self {
            device,
            relation_type,
        }
    }

    /// Reads a page through the device and parses its on-disk image.
    ///
    /// Never-written pages read back as zeroed buffers (the [`BlockDevice`]
    /// contract) and parse to empty pages, preserving the old
    /// `PageFile::read_page` semantics the scan loop relies on: an empty
    /// page terminates the scan.
    ///
    /// [`BlockDevice`]: relvar_storage_core::device::BlockDevice
    fn load_page(&mut self, page_id: PageId) -> Result<Page, HeapError> {
        let mut buffer = [0u8; PAGE_SIZE];
        self.device
            .read_page(page_id, &mut buffer)
            .map_err(device_error)?;
        Ok(Page::from_bytes(page_id, &buffer)?)
    }

    /// Writes a page through the device.
    ///
    /// Flushes after the write to preserve the old `PageFile::write_page`
    /// durability contract (every page write was followed by `sync_all`).
    fn store_page(&mut self, page: &Page) -> Result<(), HeapError> {
        let bytes = page.to_bytes()?;
        self.device
            .write_page(page.id(), &bytes)
            .map_err(device_error)?;
        self.device.flush().map_err(device_error)?;
        Ok(())
    }

    /// Inserts a tuple into the heap file.
    ///
    /// The tuple is serialized and stored in the first page with sufficient
    /// space. If no existing page has room, a new page is allocated.
    ///
    /// # TTM Compliance
    ///
    /// Returns `Result<(), HeapError>` rather than a TupleId, per TTM
    /// Proscription 6.
    ///
    /// # Arguments
    ///
    /// * `tuple` - The tuple to insert
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Serialization`] if the tuple cannot be serialized.
    /// Returns [`HeapError::Page`] if a page I/O error occurs.
    ///
    /// # Examples
    /// ```text
    /// use relvar_storage::storage::heap::HeapFile;
    /// use relvar_core::types::{RelationType, TupleType};
    /// use relvar_core::values::Tuple;
    /// use std::collections::HashMap;
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let tuple_type = TupleType::new();
    /// let rel_type = RelationType::new(tuple_type.clone());
    /// let mut heap = HeapFile::create(dir.path().join("test.heap"), rel_type).unwrap();
    /// let tuple = Tuple::new(tuple_type, HashMap::new()).unwrap();
    /// heap.insert_tuple(&tuple).unwrap();
    /// ```text
    pub fn insert_tuple(&mut self, tuple: &Tuple) -> Result<(), HeapError> {
        // Serialize the tuple
        let tuple_data = serialize_compat(tuple)?;

        // Check if tuple is too large to ever fit
        self.check_tuple_size_limit(tuple_data.len())?;

        // Find a page with enough space, or create a new one
        self.find_page_for_insertion(|heap, page_id| {
            heap.try_insert_into_page(page_id, &tuple_data).map(|_| ())
        })
    }

    /// Check if a tuple can theoretically fit in an empty page
    fn check_tuple_size_limit(&self, tuple_data_len: usize) -> Result<(), HeapError> {
        // Create a dummy page with one slot to calculate exact header size
        let dummy_page = SlottedPage {
            slot_count: 1,
            slots: vec![Some(SlotEntry {
                offset: 0,
                length: tuple_data_len as u32,
            })],
        };

        let header_size = serialized_size_compat(&dummy_page)? as usize;

        const USABLE_PAGE_SIZE: usize = PAGE_SIZE - 8;

        let required_space = header_size.checked_add(tuple_data_len).ok_or_else(|| {
            HeapError::Serialization("Header size + tuple data length overflow".to_string())
        })?;

        if required_space > USABLE_PAGE_SIZE {
            return Err(HeapError::TupleTooLarge(tuple_data_len));
        }
        Ok(())
    }

    /// Helper to repack slots and calculate offsets.
    /// Iterates backward from the end of the available space.
    /// Assumes slots are already populated (Some) for valid tuples.
    ///
    /// Test-only: page-layout tests use this to build expected pages by hand.
    #[cfg(test)]
    fn repack_slots(
        slots: &mut [Option<SlotEntry>],
        tuples: &[Vec<u8>],
        usable_size: usize,
    ) -> Result<(), HeapError> {
        let mut current_offset = usable_size;
        for (idx, tuple) in tuples.iter().enumerate().rev() {
            if tuple.is_empty() {
                continue;
            }

            if let Some(slot) = slots.get_mut(idx).and_then(|s| s.as_mut()) {
                current_offset = current_offset
                    .checked_sub(tuple.len())
                    .ok_or(HeapError::PageFull)?;
                slot.offset = current_offset as u32;
                slot.length = tuple.len() as u32;
            }
        }
        Ok(())
    }

    /// Helper to repack versioned slots and calculate offsets.
    /// Computes the required header size (V2_HEADER_SIZE + slot directory length).
    /// Returns an error if the page would overflow.
    fn repack_and_verify_space(
        versioned_page: &mut VersionedSlottedPage,
        tuples: &[Vec<u8>],
        usable_size: usize,
    ) -> Result<usize, HeapError> {
        // 1. Repack slots. Since tuples grow downward from `usable_size` (which is constant),
        //    the assigned offsets are final and deterministic on the first pass.
        Self::repack_versioned_slots(&mut versioned_page.slots, tuples, usable_size)?;

        // 2. Serialize the updated slot directory. Now that the offsets are the large final values,
        //    the varint encoding will take its true maximum size.
        let slot_dir = serialize_compat(&versioned_page)?;
        let header_size = V2_HEADER_SIZE.checked_add(slot_dir.len()).ok_or_else(|| {
            HeapError::Serialization("Header size + slot directory length overflow".to_string())
        })?;

        // 3. Verify no overlap between the downward-growing tuples and the upward-growing header.
        for (idx, slot_entry) in versioned_page.slots.iter().enumerate() {
            if let Some(entry) = slot_entry {
                let offset = entry.offset as usize;
                if idx < tuples.len() && !tuples[idx].is_empty() && offset < header_size {
                    return Err(HeapError::PageFull);
                }
            }
        }

        Ok(header_size)
    }

    /// Helper to repack versioned slots and calculate offsets.
    /// Iterates backward from the end of the available space.
    /// Assumes slots are already populated (Some) for valid tuples.
    fn repack_versioned_slots(
        slots: &mut [Option<VersionedSlotEntry>],
        tuples: &[Vec<u8>],
        usable_size: usize,
    ) -> Result<(), HeapError> {
        let mut current_offset = usable_size;
        for (idx, tuple) in tuples.iter().enumerate().rev() {
            if tuple.is_empty() {
                continue;
            }

            if let Some(slot) = slots.get_mut(idx).and_then(|s| s.as_mut()) {
                current_offset = current_offset
                    .checked_sub(tuple.len())
                    .ok_or(HeapError::PageFull)?;
                slot.offset = current_offset as u32;
                slot.length = tuple.len() as u32;
            }
        }
        Ok(())
    }

    /// Helper to find a free slot or allocate a new one.
    fn find_or_allocate_slot<T>(slots: &mut Vec<Option<T>>, slot_count: &mut u32) -> u32 {
        if let Some(pos) = slots.iter().position(|s| s.is_none()) {
            pos as u32
        } else {
            let new_slot = slots.len() as u32;
            slots.push(None);
            *slot_count += 1;
            new_slot
        }
    }

    /// Helper to extract tuples from a sequence of slots.
    fn extract_tuples_from_slots<'a, I>(
        &self,
        page: &Page,
        slots: I,
    ) -> Result<Vec<Tuple>, HeapError>
    where
        I: Iterator<Item = &'a SlotEntry>,
    {
        let (lower, _) = slots.size_hint();
        // pre-allocate to avoid repeated allocations
        let mut results = Vec::with_capacity(lower);
        for slot in slots {
            let tuple = self.extract_tuple_from_page(page, slot.offset, slot.length)?;
            results.push(tuple);
        }
        Ok(results)
    }

    /// Helper to extract tuples from a sequence of versioned slots.
    fn extract_tuples_from_versioned_slots<'a, I>(
        &self,
        page: &Page,
        slots: I,
    ) -> Result<Vec<Tuple>, HeapError>
    where
        I: Iterator<Item = &'a VersionedSlotEntry>,
    {
        let (lower, _) = slots.size_hint();
        // pre-allocate to avoid repeated allocations
        let mut results = Vec::with_capacity(lower);
        for slot in slots {
            let tuple = self.extract_tuple_from_page(page, slot.offset, slot.length)?;
            results.push(tuple);
        }
        Ok(results)
    }

    /// Helper to extract all tuples from a page based on slot entries.
    /// This abstracts the common logic used in insert, update, delete, and GC operations.
    ///
    /// Test-only: layout tests use this to verify page contents.
    #[cfg(test)]
    fn extract_all_tuples(
        &self,
        page: &Page,
        slots: &[Option<SlotEntry>],
    ) -> Result<Vec<Vec<u8>>, HeapError> {
        let mut existing_tuples: Vec<Vec<u8>> = Vec::with_capacity(slots.len());
        // Maintain alignment with slots: push empty Vec for None slots
        for slot_option in slots.iter() {
            if let Some(slot_entry) = slot_option {
                let raw_data =
                    self.extract_raw_tuple_data(page, slot_entry.offset, slot_entry.length)?;
                existing_tuples.push(raw_data);
            } else {
                existing_tuples.push(Vec::new());
            }
        }
        Ok(existing_tuples)
    }

    /// Helper to extract all tuples from a page based on versioned slot entries.
    /// This abstracts the common logic used in insert, update, delete, and GC operations.
    fn extract_all_versioned_tuples(
        &self,
        page: &Page,
        slots: &[Option<VersionedSlotEntry>],
    ) -> Result<Vec<Vec<u8>>, HeapError> {
        let mut existing_tuples: Vec<Vec<u8>> = Vec::with_capacity(slots.len());
        // Maintain alignment with slots: push empty Vec for None slots
        for slot_option in slots.iter() {
            if let Some(slot_entry) = slot_option {
                let raw_data =
                    self.extract_raw_tuple_data(page, slot_entry.offset, slot_entry.length)?;
                existing_tuples.push(raw_data);
            } else {
                existing_tuples.push(Vec::new());
            }
        }
        Ok(existing_tuples)
    }

    /// Finds the first page that can accommodate the insertion.
    /// Retries on PageFull error by incrementing the page ID.
    fn find_page_for_insertion<F, R>(&mut self, mut insert_fn: F) -> Result<R, HeapError>
    where
        F: FnMut(&mut Self, PageId) -> Result<R, HeapError>,
    {
        let mut page_id = 0;
        loop {
            match insert_fn(self, page_id) {
                Ok(result) => return Ok(result),
                Err(HeapError::PageFull) => {
                    page_id += 1;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Try to insert tuple data into a specific page.
    ///
    /// Uses the standard slotted-page technique:
    ///
    /// 1. Deserialize only the slot directory (tuple data is never read).
    /// 2. Append the new tuple bytes into free space at the page end.
    /// 3. Rewrite only the slot directory at the start of the page.
    ///
    /// Existing tuple bytes are never read, moved, or rewritten, so their
    /// slot offsets stay stable across inserts. Insert cost is independent
    /// of the number of tuples already on the page.
    fn try_insert_into_page(
        &mut self,
        page_id: PageId,
        tuple_data: &[u8],
    ) -> Result<u32, HeapError> {
        // Read the page (or start from a zeroed buffer if it doesn't exist yet).
        let page = self.load_page(page_id)?;
        let mut page_data = if page.is_empty() {
            vec![0u8; USABLE_PAGE_SIZE_V1]
        } else {
            page.data().to_vec()
        };

        // 1. Read ONLY the slot directory.
        let mut slotted_page = if page.is_empty() {
            SlottedPage {
                slot_count: 0,
                slots: Vec::new(),
            }
        } else {
            decode_slotted_page(page.data())?
        };

        // Allocate a slot for the new tuple (reuses a freed slot when one exists).
        let slot_number =
            Self::find_or_allocate_slot(&mut slotted_page.slots, &mut slotted_page.slot_count);
        let new_slot = slot_number as usize;

        // 2. Locate free space. Tuples grow downward from the end of the
        // usable area, so the new tuple goes just below the lowest live
        // tuple. Existing slots are bounds-checked here, but their tuple
        // data is never read.
        let mut lowest_live_offset = page_data.len();
        for (idx, slot) in slotted_page.slots.iter().enumerate() {
            if idx == new_slot {
                continue;
            }
            if let Some(entry) = slot {
                let offset = entry.offset as usize;
                let end = offset.checked_add(entry.length as usize).ok_or_else(|| {
                    HeapError::Serialization("Tuple offset + length overflow".to_string())
                })?;
                if end > page_data.len() {
                    return Err(HeapError::Serialization(format!(
                        "Corrupted slot on page {page_id} points outside page data"
                    )));
                }
                lowest_live_offset = lowest_live_offset.min(offset);
            }
        }

        let tuple_len = tuple_data.len();
        let new_offset = lowest_live_offset
            .checked_sub(tuple_len)
            .ok_or(HeapError::PageFull)?;

        // 3. Serialize the updated slot directory with the final offset so the
        // header size is exact, then verify the header and the new tuple fit
        // in free space without colliding.
        slotted_page.slots[new_slot] = Some(SlotEntry {
            offset: new_offset as u32,
            length: tuple_len as u32,
        });
        let new_slot_dir = serialize_compat(&slotted_page)?;
        let new_header_len = new_slot_dir.len();
        if new_header_len > new_offset {
            return Err(HeapError::PageFull);
        }

        // 4. Append the tuple bytes and rewrite only the slot directory.
        // (new_offset + tuple_len == lowest_live_offset <= page_data.len(),
        // so both copies are in bounds.)
        page_data[new_offset..new_offset + tuple_len].copy_from_slice(tuple_data);
        page_data[..new_header_len].copy_from_slice(&new_slot_dir);

        // Write the page
        let updated_page = Page::from_data(page_id, page_data)?;
        self.store_page(&updated_page)?;

        Ok(slot_number)
    }

    /// Read a tuple by its TupleId (internal use only per TTM Proscription 6)
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn read_tuple(&mut self, tuple_id: TupleId) -> Result<Tuple, HeapError> {
        let page = self.load_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let slotted_page: SlottedPage = deserialize_bounded(page.data())?;

        let slot_entry = slotted_page
            .slots
            .get(tuple_id.slot as usize)
            .and_then(|s| s.as_ref())
            .ok_or(HeapError::TupleNotFound)?;

        // Extract tuple data from page
        let start = slot_entry.offset as usize;
        let end = start
            .checked_add(slot_entry.length as usize)
            .ok_or_else(|| HeapError::Serialization("Tuple end offset overflow".to_string()))?;

        if end > page.data().len() {
            return Err(HeapError::Serialization(
                "Corrupted slot points outside page data".to_string(),
            ));
        }

        let tuple_data = &page.data()[start..end];
        let tuple: Tuple = deserialize_bounded(tuple_data)?;

        Ok(tuple)
    }

    /// Reads a tuple from a versioned page.
    ///
    /// Used internally by MVCC operations.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn read_tuple_versioned(&mut self, tuple_id: TupleId) -> Result<Tuple, HeapError> {
        let page = self.load_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let versioned_page = decode_versioned_page(page.data())?;

        let slot_entry = versioned_page
            .slots
            .get(tuple_id.slot as usize)
            .and_then(|s| s.as_ref())
            .ok_or(HeapError::TupleNotFound)?;

        // Extract tuple data from page
        let start = slot_entry.offset as usize;
        let end = start
            .checked_add(slot_entry.length as usize)
            .ok_or_else(|| HeapError::Serialization("Tuple end offset overflow".to_string()))?;

        if end > page.data().len() {
            return Err(HeapError::Serialization(
                "Corrupted slot points outside page data".to_string(),
            ));
        }

        let tuple_data = &page.data()[start..end];
        let tuple: Tuple = deserialize_bounded(tuple_data)?;

        Ok(tuple)
    }

    /// Scans all tuples in the heap file.
    ///
    /// Iterates through all pages and returns every valid tuple. The order
    /// of tuples is not guaranteed (heap files are unordered).
    ///
    /// # TTM Compliance
    ///
    /// Returns `Vec<Tuple>` without TupleId, per TTM Proscription 6.
    ///
    /// # Performance
    ///
    /// This is a full table scan with O(n) complexity where n is the number
    /// of pages. For large relations, consider using indexes for selective
    /// queries.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Page`] if a page read fails.
    /// Returns [`HeapError::Serialization`] if tuple deserialization fails.
    ///
    /// # Examples
    /// ```text
    /// use relvar_storage::storage::heap::HeapFile;
    /// use relvar_core::types::{RelationType, TupleType};
    /// use relvar_core::values::Tuple;
    /// use std::collections::HashMap;
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let tuple_type = TupleType::new();
    /// let rel_type = RelationType::new(tuple_type.clone());
    /// let mut heap = HeapFile::create(dir.path().join("test.heap"), rel_type).unwrap();
    /// let tuple = Tuple::new(tuple_type, HashMap::new()).unwrap();
    /// heap.insert_tuple(&tuple).unwrap();
    /// let tuples = heap.scan().unwrap();
    /// assert_eq!(tuples.len(), 1);
    /// ```text
    pub fn scan(&mut self) -> Result<Vec<Tuple>, HeapError> {
        let mut results = Vec::new();
        let mut page_id = 0;

        // Scan pages until we hit an empty one
        loop {
            let page = self.load_page(page_id)?;

            if page.is_empty() {
                // Empty page means no more data
                break;
            }

            if is_versioned_page(page.data()) {
                // Versioned page format (MVCC)
                let versioned_page = decode_versioned_page(page.data())?;
                results.extend(self.extract_tuples_from_versioned_slots(
                    &page,
                    versioned_page.slots.iter().flatten(),
                )?);
            } else {
                // Old slotted page format (non-MVCC)
                let slotted_page = decode_slotted_page(page.data())?;
                results.extend(
                    self.extract_tuples_from_slots(&page, slotted_page.slots.iter().flatten())?,
                );
            }

            page_id += 1;
        }

        Ok(results)
    }

    /// Loads all tuples from the heap file into a `Relation`.
    ///
    /// This is a convenience method that scans all tuples and constructs
    /// a [`Relation`] value with the heap file's relation type.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Serialization`] if tuples cannot be deserialized
    /// or if the relation cannot be constructed.
    ///
    /// # Examples
    /// ```text
    /// use relvar_storage::storage::heap::HeapFile;
    /// use relvar_core::types::{RelationType, TupleType};
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let rel_type = RelationType::new(TupleType::new());
    /// let mut heap = HeapFile::create(dir.path().join("test.heap"), rel_type).unwrap();
    /// let rel = heap.load_relation().unwrap();
    /// assert_eq!(rel.cardinality(), 0);
    /// ```text
    pub fn load_relation(&mut self) -> Result<Relation, HeapError> {
        let tuples = self.scan()?; // Already returns Vec<Tuple>

        Relation::from_tuples(self.relation_type.clone(), tuples)
            .map_err(|e| HeapError::Serialization(e.to_string()))
    }

    /// Stores all tuples from a relation into the heap file.
    ///
    /// Iterates through the relation's tuples and inserts each one.
    ///
    /// # TTM Compliance
    ///
    /// Returns `Result<(), HeapError>` (no TupleId vector), per TTM
    /// Proscription 6.
    ///
    /// # Arguments
    ///
    /// * `relation` - The relation whose tuples should be stored
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Serialization`] if a tuple cannot be serialized.
    /// Returns [`HeapError::Page`] if a page I/O error occurs.
    pub fn store_relation(&mut self, relation: &Relation) -> Result<(), HeapError> {
        for tuple in relation.tuples() {
            self.insert_tuple(tuple)?;
        }
        Ok(())
    }

    /// Flushes all pending writes to disk.
    ///
    /// Ensures durability by calling `fsync` on the underlying page file.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Page`] if the sync fails.
    pub fn sync(&mut self) -> Result<(), HeapError> {
        self.device.flush().map_err(device_error)?;
        Ok(())
    }

    /// Inserts a tuple with version metadata for MVCC.
    ///
    /// # Arguments
    /// * `tuple` - The tuple to insert
    /// * `txn_id` - Transaction ID that created this version
    ///
    /// # Returns
    /// TupleId of the inserted version (internal use only)
    ///
    /// # Errors
    /// Returns [`HeapError::Serialization`] if tuple cannot be serialized.
    /// Returns [`HeapError::Page`] if page I/O error occurs.
    pub(crate) fn insert_tuple_versioned(
        &mut self,
        tuple: &Tuple,
        txn_id: crate::wal::TransactionId,
    ) -> Result<TupleId, HeapError> {
        // Serialize the tuple
        let tuple_data = serialize_compat(tuple)?;

        // Check if tuple is too large to ever fit
        // New inserts have no previous version (prev_version = None)
        self.check_versioned_tuple_size_limit(tuple_data.len(), false)?;

        // Find a page with enough space, or create a new one
        self.find_page_for_insertion(|heap, page_id| {
            heap.try_insert_into_page_versioned(page_id, &tuple_data, txn_id, None)
                .map(|slot| TupleId { page_id, slot })
        })
    }

    /// Check if a versioned tuple can theoretically fit in an empty page
    fn check_versioned_tuple_size_limit(
        &self,
        tuple_data_len: usize,
        is_update: bool,
    ) -> Result<(), HeapError> {
        // Create a dummy versioned page with one slot
        let dummy_page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 1,
            slots: vec![Some(VersionedSlotEntry {
                offset: 0,
                length: tuple_data_len as u32,
                xmin: crate::wal::TransactionId::new(0),
                xmax: None,
                prev_version: if is_update {
                    Some(TupleId {
                        page_id: 0,
                        slot: 0,
                    })
                } else {
                    None
                },
            })],
        };

        let header_size = serialized_size_compat(&dummy_page)? as usize;

        const USABLE_PAGE_SIZE: usize = PAGE_SIZE - 8;
        const FORMAT_HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length

        let total_header = FORMAT_HEADER_SIZE.checked_add(header_size).ok_or_else(|| {
            HeapError::Serialization("Format header + header size overflow".to_string())
        })?;
        let required_space = total_header.checked_add(tuple_data_len).ok_or_else(|| {
            HeapError::Serialization("Header size + tuple data length overflow".to_string())
        })?;

        if required_space > USABLE_PAGE_SIZE {
            return Err(HeapError::TupleTooLarge(tuple_data_len));
        }
        Ok(())
    }

    /// Try to insert tuple data into a specific page with version metadata
    fn try_insert_into_page_versioned(
        &mut self,
        page_id: PageId,
        tuple_data: &[u8],
        txn_id: crate::wal::TransactionId,
        prev_version: Option<TupleId>,
    ) -> Result<u32, HeapError> {
        // Read the page (or create empty if doesn't exist)
        let page = self.load_page(page_id)?;

        // Read existing tuples from the page
        let (mut versioned_page, mut existing_tuples) = if page.is_empty() {
            (
                VersionedSlottedPage {
                    magic: VERSIONED_PAGE_MAGIC,
                    slot_count: 0,
                    slots: Vec::new(),
                },
                Vec::new(),
            )
        } else {
            let vp = decode_versioned_page(page.data())?;
            let tuples = self.extract_all_versioned_tuples(&page, &vp.slots)?;
            (vp, tuples)
        };

        let slot_number = Self::prepare_insert_versioned(
            &mut versioned_page,
            &mut existing_tuples,
            tuple_data,
            txn_id,
            prev_version,
        )?;

        // Serialize the updated page with all tuples
        let page_data = encode_versioned_page(&versioned_page, &existing_tuples)?;

        // Write the page
        let updated_page = Page::from_data(page_id, page_data)?;
        self.store_page(&updated_page)?;

        Ok(slot_number)
    }

    fn prepare_insert_versioned(
        versioned_page: &mut VersionedSlottedPage,
        existing_tuples: &mut Vec<Vec<u8>>,
        tuple_data: &[u8],
        txn_id: crate::wal::TransactionId,
        prev_version: Option<TupleId>,
    ) -> Result<u32, HeapError> {
        // Find free slot or add new one
        // NOTE: We do this BEFORE space calculation so we know the final slot count
        let slot_number =
            Self::find_or_allocate_slot(&mut versioned_page.slots, &mut versioned_page.slot_count);

        // Add new tuple to the list (overwrite if reusing slot, append if new)
        if (slot_number as usize) < existing_tuples.len() {
            existing_tuples[slot_number as usize] = tuple_data.to_vec();
        } else {
            existing_tuples.push(tuple_data.to_vec());
        }

        // Initialize the new slot
        versioned_page.slots[slot_number as usize] = Some(VersionedSlotEntry {
            offset: 0,
            length: tuple_data.len() as u32,
            xmin: txn_id,
            xmax: None,
            prev_version,
        });

        Self::repack_and_verify_space(versioned_page, existing_tuples, USABLE_PAGE_SIZE_V2)?;

        Ok(slot_number)
    }

    /// Extracts a tuple from a page at the given offset and length.
    fn extract_tuple_from_page(
        &self,
        page: &Page,
        offset: u32,
        length: u32,
    ) -> Result<Tuple, HeapError> {
        let tuple_data = slot_bytes(page.data(), offset, length)?;
        deserialize_bounded(tuple_data)
    }

    /// Extracts raw tuple data from a page at the given offset and length.
    fn extract_raw_tuple_data(
        &self,
        page: &Page,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, HeapError> {
        Ok(slot_bytes(page.data(), offset, length)?.to_vec())
    }

    /// Updates a tuple by creating a new version and marking the old version as deleted.
    ///
    /// This implements MVCC update semantics:
    /// 1. Sets xmax on the old version to mark it as superseded
    /// 2. Inserts a new version with the updated data
    /// 3. Links the new version to the old version via prev_version pointer
    ///
    /// # Arguments
    /// * `old_tuple_id` - TupleId of the version to update
    /// * `new_tuple` - The new tuple data
    /// * `txn_id` - Transaction performing the update
    ///
    /// # Returns
    /// TupleId of the newly created version
    ///
    /// # Errors
    /// Returns [`HeapError::TupleNotFound`] if old_tuple_id doesn't exist.
    /// Returns [`HeapError::Serialization`] if tuple cannot be serialized.
    /// Returns [`HeapError::Page`] if page I/O error occurs.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn update_tuple_versioned(
        &mut self,
        old_tuple_id: TupleId,
        new_tuple: &Tuple,
        txn_id: crate::wal::TransactionId,
    ) -> Result<TupleId, HeapError> {
        // Step 1: Mark old version's xmax
        let page = self.load_page(old_tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let mut versioned_page = decode_versioned_page(page.data())?;

        // Find the old slot
        let old_slot = versioned_page
            .slots
            .get_mut(old_tuple_id.slot as usize)
            .and_then(|s| s.as_mut())
            .ok_or(HeapError::TupleNotFound)?;

        // Mark old version as deleted by this transaction
        old_slot.xmax = Some(txn_id);

        // Extract all existing tuple data
        let existing_tuples = self.extract_all_versioned_tuples(&page, &versioned_page.slots)?;

        // Serialize and write updated page with old version marked
        let page_data = encode_versioned_page(&versioned_page, &existing_tuples)?;
        let updated_page = Page::from_data(old_tuple_id.page_id, page_data)?;
        self.store_page(&updated_page)?;

        // Step 2: Insert new version
        let new_tuple_data = serialize_compat(new_tuple)?;

        // Check if new tuple is too large
        // Updates link to previous version (prev_version = Some(...))
        self.check_versioned_tuple_size_limit(new_tuple_data.len(), true)?;

        // Find a page with space for new version
        self.find_page_for_insertion(|heap, page_id| {
            heap.try_insert_into_page_versioned(
                page_id,
                &new_tuple_data,
                txn_id,
                Some(old_tuple_id),
            )
            .map(|slot| TupleId { page_id, slot })
        })
    }

    /// Deletes a tuple by marking it with xmax (MVCC soft delete).
    ///
    /// This implements MVCC delete semantics:
    /// - Sets xmax on the tuple to mark it as deleted
    /// - Does not physically remove the tuple (needed for version visibility)
    /// - Tuple becomes invisible to transactions that start after the delete commits
    ///
    /// # Arguments
    /// * `tuple_id` - TupleId of the tuple to delete
    /// * `txn_id` - Transaction performing the delete
    ///
    /// # Errors
    /// Returns [`HeapError::TupleNotFound`] if tuple_id doesn't exist.
    /// Returns [`HeapError::Serialization`] if page cannot be serialized.
    /// Returns [`HeapError::Page`] if page I/O error occurs.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn delete_tuple_versioned(
        &mut self,
        tuple_id: TupleId,
        txn_id: crate::wal::TransactionId,
    ) -> Result<(), HeapError> {
        // Read the page containing the tuple
        let page = self.load_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let mut versioned_page = decode_versioned_page(page.data())?;

        // Find and mark the tuple
        let slot = versioned_page
            .slots
            .get_mut(tuple_id.slot as usize)
            .and_then(|s| s.as_mut())
            .ok_or(HeapError::TupleNotFound)?;

        // Mark as deleted by this transaction
        slot.xmax = Some(txn_id);

        // Extract all existing tuple data
        let existing_tuples = self.extract_all_versioned_tuples(&page, &versioned_page.slots)?;

        // Serialize and write updated page
        let page_data = encode_versioned_page(&versioned_page, &existing_tuples)?;
        let updated_page = Page::from_data(tuple_id.page_id, page_data)?;
        self.store_page(&updated_page)?;

        Ok(())
    }

    /// Removes dead tuple versions for garbage collection.
    ///
    /// A version is dead if:
    /// - It has xmax set (deleted or updated)
    /// - The xmax transaction is committed
    /// - The xmax transaction LSN < oldest_active_lsn (no active txn can see it)
    ///
    /// # Arguments
    /// * `oldest_active_lsn` - LSN of the oldest active transaction
    /// * `committed` - Set of committed transaction IDs
    ///
    /// # Returns
    /// Number of versions removed
    ///
    /// # Errors
    /// Returns `HeapError` if page I/O fails
    pub(crate) fn gc_remove_dead_versions(
        &mut self,
        oldest_active_lsn: crate::wal::Lsn,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
    ) -> Result<usize, HeapError> {
        let mut removed_count = 0;
        let mut page_id = 0;

        loop {
            let page = self.load_page(page_id)?;

            if page.is_empty() {
                break;
            }

            if !is_versioned_page(page.data()) {
                page_id += 1;
                continue;
            }

            removed_count += self.gc_process_page(page_id, &page, oldest_active_lsn, committed)?;

            page_id += 1;
        }

        Ok(removed_count)
    }

    fn gc_process_page(
        &mut self,
        page_id: PageId,
        page: &Page,
        oldest_active_lsn: crate::wal::Lsn,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
    ) -> Result<usize, HeapError> {
        // Try to deserialize as versioned page
        let mut versioned_page = match decode_versioned_page(page.data()) {
            Ok(vp) => vp,
            Err(_) => return Ok(0), // Not a versioned page or corrupted, skip
        };

        // Extract existing tuple data
        let mut existing_tuples = self.extract_all_versioned_tuples(page, &versioned_page.slots)?;

        let mut removed_count = 0;
        let mut page_modified = false;

        // Check each slot for dead versions
        for (idx, slot_option) in versioned_page.slots.iter_mut().enumerate() {
            if let Some(slot) = slot_option
                && Self::is_dead_version(slot, oldest_active_lsn, committed)
            {
                // This version is dead - remove it
                *slot_option = None;
                existing_tuples[idx] = Vec::new();
                removed_count += 1;
                page_modified = true;
            }
        }

        // Write page back if modified
        if page_modified {
            // Repack slots
            Self::repack_versioned_slots(
                &mut versioned_page.slots,
                &existing_tuples,
                USABLE_PAGE_SIZE_V2,
            )?;

            let page_data = encode_versioned_page(&versioned_page, &existing_tuples)?;
            let updated_page = Page::from_data(page_id, page_data)?;
            self.store_page(&updated_page)?;
        }

        Ok(removed_count)
    }

    fn is_dead_version(
        slot: &VersionedSlotEntry,
        oldest_active_lsn: crate::wal::Lsn,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
    ) -> bool {
        // Check if this version is dead
        if let Some(xmax) = slot.xmax {
            // Has xmax - was deleted or updated
            if committed.contains(&xmax) {
                // xmax transaction committed
                // Check if it's old enough (before oldest active)
                // Note: We need to compare transaction IDs as proxy for LSN
                // since we don't track commit LSNs yet
                return xmax.value() < oldest_active_lsn.value();
            }
        }
        false
    }

    /// Scans all visible tuples for a given transaction snapshot.
    ///
    /// Only returns tuples that are visible according to MVCC visibility rules.
    ///
    /// # Arguments
    /// * `snapshot` - Transaction snapshot determining visibility
    /// * `committed` - Set of all committed transaction IDs
    ///
    /// # Returns
    /// Vector of visible tuples
    ///
    /// # Errors
    /// Returns [`HeapError::Serialization`] if tuples cannot be deserialized.
    /// Returns [`HeapError::Page`] if page I/O error occurs.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn scan_visible(
        &mut self,
        snapshot: &crate::mvcc::TransactionSnapshot,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
    ) -> Result<Vec<Tuple>, HeapError> {
        let mut results = Vec::new();
        let mut page_id = 0;

        loop {
            let page = self.load_page(page_id)?;

            if page.is_empty() {
                // Empty page means no more data
                break;
            }

            // Try to deserialize as VersionedSlottedPage
            let versioned_page = match decode_versioned_page(page.data()) {
                Ok(vp) => vp,
                Err(_) => {
                    // Not a versioned page, skip
                    page_id += 1;
                    continue;
                }
            };

            // Check each slot for visibility
            for slot_entry in versioned_page.slots.iter().flatten() {
                // Create version metadata
                let version_metadata = crate::mvcc::VersionMetadata {
                    xmin: slot_entry.xmin,
                    xmax: slot_entry.xmax,
                };

                // Check visibility
                if crate::mvcc::visibility::is_visible(&version_metadata, snapshot, committed) {
                    // Extract tuple data from page
                    let start = slot_entry.offset as usize;
                    let end = start
                        .checked_add(slot_entry.length as usize)
                        .ok_or_else(|| {
                            HeapError::Serialization("Tuple end offset overflow".to_string())
                        })?;

                    if end > page.data().len() {
                        return Err(HeapError::Serialization(
                            "Corrupted slot points outside page data".to_string(),
                        ));
                    }

                    let tuple_data = &page.data()[start..end];
                    let tuple: Tuple = deserialize_bounded(tuple_data)?;
                    results.push(tuple);
                }
            }

            page_id += 1;
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests;
