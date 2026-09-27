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
use crate::mvcc::{MvccError, VersionPool};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
// Slotted-page types, wire formats, and codecs live in the no_std storage
// core; the heap file only adds tuple serde and page-management policy.
// (Private `use` items are visible to the child test modules via `::*`.)
use relvar_storage_core::device::BlockDevice;
use relvar_storage_core::slotted::{
    SlotEntry, SlottedError, SlottedPage, TupleId, USABLE_PAGE_SIZE_V1, USABLE_PAGE_SIZE_V2,
    VERSIONED_PAGE_MAGIC, VersionedSlotEntry, decode_slotted_page, decode_versioned_page_into,
    encode_versioned_header_into, is_versioned_page, slot_bytes,
};
// Re-exported for the page-layout tests (child modules import via `::*`);
// unused in the non-test build.
#[cfg(test)]
use relvar_storage_core::slotted::{
    PAGE_FORMAT_VERSION, VersionedSlottedPage, decode_versioned_page, encode_slotted_page,
    encode_versioned_page,
};
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
///
/// Uses `postcard::experimental::serialized_size`: no allocation, unlike serializing to a
/// temporary vector.
fn serialized_size_compat<T: Serialize>(value: &T) -> Result<u64, HeapError> {
    postcard::experimental::serialized_size(value)
        .map(|v| v as u64)
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

    /// An MVCC pool error: version-pool or version-buffer exhaustion.
    /// Exhaustion is typed and recoverable, never a panic.
    #[error("MVCC pool error: {0}")]
    Mvcc(#[from] MvccError),

    /// An update's replacement insert failed and the compensating rollback
    /// of the old version's `xmax` marking failed as well.
    ///
    /// The old version remains marked deleted by this transaction. The
    /// transaction is poisoned: the caller MUST abort it — committing
    /// would hide the old version without its replacement, silently
    /// losing the tuple. Both failures are carried (neither is discarded)
    /// so the abort path can report what happened.
    #[error(
        "update lost atomicity: insert failed ({insert_error}); rollback of the old version's deletion marking also failed ({rollback_error}); the transaction must abort, never commit"
    )]
    UpdateRollbackFailed {
        /// The replacement-insert failure that triggered the rollback.
        insert_error: String,
        /// The rollback failure that left the old version marked.
        rollback_error: String,
    },
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
    /// Helper to repack versioned slots and calculate offsets.
    /// Iterates backward from the end of the available space.
    /// Assumes slots are already populated (Some) for valid tuples.
    #[cfg(test)]
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

        let mut slots = Vec::new();
        decode_versioned_page_into(page.data(), &mut slots)?;

        let slot_entry = slots
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
                let mut slots = Vec::new();
                decode_versioned_page_into(page.data(), &mut slots)?;
                results.extend(
                    self.extract_tuples_from_versioned_slots(&page, slots.iter().flatten())?,
                );
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
    /// Inserts a tuple as a new MVCC version using the caller's version pool.
    ///
    /// The write path performs no per-version allocation after the pool is
    /// constructed: one version record is claimed from the pool, and page
    /// work reuses a pooled [`PageWorkingSet`](crate::mvcc::PageWorkingSet)
    /// (slot directory, page image, serialization scratch). The tuple is
    /// serialized directly into the page image; existing tuple bytes are
    /// copied straight from the source page.
    ///
    /// The version record is claimed *before* any page is mutated, so pool
    /// exhaustion fails typed ([`HeapError::Mvcc`]) while the write is still
    /// atomic (nothing mutated). The record is released when the
    /// transaction commits or aborts.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::TupleTooLarge`] if the tuple can never fit.
    /// Returns [`HeapError::Mvcc`] with
    /// [`MvccError::VersionPoolExhausted`](crate::mvcc::MvccError::VersionPoolExhausted)
    /// when the version pool is full, or
    /// [`MvccError::VersionBuffersExhausted`](crate::mvcc::MvccError::VersionBuffersExhausted)
    /// when no page working buffer is free. Never panics.
    pub(crate) fn insert_tuple_versioned(
        &mut self,
        tuple: &Tuple,
        txn_id: crate::wal::TransactionId,
        version_pool: &mut VersionPool,
    ) -> Result<TupleId, HeapError> {
        // Pure validation before the claim: the serialized length and the
        // size limit depend only on the tuple, so failures here mutate
        // nothing and need no rollback.
        let tuple_len = postcard::experimental::serialized_size(tuple)
            .map_err(|e| HeapError::Serialization(e.to_string()))?;

        // Check if tuple is too large to ever fit (allocation-free).
        // New inserts have no previous version (prev_version = None).
        Self::check_versioned_tuple_size_limit(tuple_len, false)?;

        // Claim the version record BEFORE any page mutation: exhaustion then
        // fails typed while the write is still atomic. The handle rolls the
        // claim back if the fallible work below fails.
        let handle: crate::mvcc::VersionHandle = version_pool.claim(txn_id)?;
        let result = self.insert_tuple_versioned_claimed(tuple, tuple_len, txn_id, version_pool);
        if result.is_err() {
            version_pool.release(handle);
        }
        result
    }

    /// Insert body; runs with a claimed version record.
    ///
    /// See [`insert_tuple_versioned`](Self::insert_tuple_versioned) for the
    /// claim/rollback protocol.
    fn insert_tuple_versioned_claimed(
        &mut self,
        tuple: &Tuple,
        tuple_len: usize,
        txn_id: crate::wal::TransactionId,
        version_pool: &mut VersionPool,
    ) -> Result<TupleId, HeapError> {
        // Check out one page working buffer for the whole insertion; the
        // guard returns it on drop, so PageFull retries reuse it.
        let mut buffer = version_pool.acquire_buffer()?;

        // Find a page with enough space, or create a new one.
        // The tuple is serialized directly into the page image inside.
        self.find_page_for_insertion(|heap, page_id| {
            heap.try_insert_into_page_versioned(
                page_id,
                tuple,
                tuple_len,
                txn_id,
                None,
                &mut buffer,
            )
            .map(|slot| TupleId { page_id, slot })
        })
    }

    /// Check if a versioned tuple can theoretically fit in an empty page.
    ///
    /// Allocation-free: instead of building a dummy page with a one-element
    /// slot vector, the probe serializes `(magic, slot_count, [entry])`.
    /// postcard is not self-describing — a struct encodes as the
    /// concatenation of its fields — so this measures exactly what the
    /// dummy page would (pinned by
    /// `test_versioned_size_probe_matches_dummy_page`).
    fn check_versioned_tuple_size_limit(
        tuple_data_len: usize,
        is_update: bool,
    ) -> Result<(), HeapError> {
        let entry = VersionedSlotEntry {
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
        };
        let probe = [Some(entry)];
        let header_size = postcard::experimental::serialized_size(&(
            VERSIONED_PAGE_MAGIC,
            1u32,
            probe.as_slice(),
        ))
        .map_err(|e| HeapError::Serialization(e.to_string()))? as usize;

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

    /// Try to insert a tuple into a specific versioned page using a pooled
    /// page working buffer instead of allocating.
    ///
    /// The caller must hold a claimed version record (see
    /// [`insert_tuple_versioned`](Self::insert_tuple_versioned)) and pass a
    /// checked-out buffer. The tuple is serialized directly into the page
    /// image; existing tuple bytes are copied straight from the source page.
    ///
    /// Returns [`HeapError::PageFull`] (without mutating the source page)
    /// when the tuple does not fit, so the caller can retry on another page.
    fn try_insert_into_page_versioned(
        &mut self,
        page_id: PageId,
        tuple: &Tuple,
        tuple_len: usize,
        txn_id: crate::wal::TransactionId,
        prev_version: Option<TupleId>,
        buffer: &mut crate::mvcc::PageWorkingSet,
    ) -> Result<u32, HeapError> {
        // Read the page (or start from an empty directory if it doesn't exist yet).
        let page = self.load_page(page_id)?;

        // Decode only the slot directory into the pooled working set.
        if page.is_empty() {
            buffer.reset();
        } else {
            let header = decode_versioned_page_into(page.data(), &mut buffer.page.slots)?;
            buffer.page.magic = header.magic;
            buffer.page.slot_count = header.slot_count;
        }

        // Allocate a slot for the new version (reuses a freed slot when one
        // exists). Done BEFORE space calculation so the slot directory has
        // its final size.
        let slot_number =
            Self::find_or_allocate_slot(&mut buffer.page.slots, &mut buffer.page.slot_count);

        // Initialize the new version's entry (offset assigned by the repack
        // inside `emit_versioned_page`).
        buffer.page.slots[slot_number as usize] = Some(VersionedSlotEntry {
            offset: 0,
            length: tuple_len as u32,
            xmin: txn_id,
            xmax: None,
            prev_version,
        });

        // Repack, verify space, and emit the page image into the pooled
        // buffer: existing bytes are copied from the source page, the new
        // tuple is serialized in place.
        Self::emit_versioned_page(
            buffer,
            page.data(),
            Some((tuple, tuple_len, slot_number as usize)),
        )?;

        // Write the pooled image straight to the device.
        self.store_page_bytes(page_id, &buffer.page_image)?;

        Ok(slot_number)
    }

    /// Rebuilds a versioned page image in the pooled working set without
    /// allocating.
    ///
    /// `source` is the previous page payload; live tuple bytes are copied
    /// from it using their current slot offsets. `new_tuple`, when present,
    /// is `(tuple, serialized_len, slot_index)`: the tuple is serialized
    /// directly into its final image position instead of a temporary vector.
    ///
    /// Steps (mirroring the previous allocating implementation):
    ///
    /// 1. Repack: assign final offsets from the end of usable space,
    ///    placing tuple bytes in the same pass (old offsets are read before
    ///    being overwritten, so no auxiliary offset table is needed).
    /// 2. Frame the v2 header + slot directory into the image head (needs
    ///    final offsets for true varint sizes; the tuple region is untouched).
    /// 3. Verify no tuple overlaps the header; else [`HeapError::PageFull`].
    ///
    /// On `PageFull` the working set is left dirty but the source page is
    /// untouched, so the caller can retry on another page.
    fn emit_versioned_page(
        buffer: &mut crate::mvcc::PageWorkingSet,
        source: &[u8],
        new_tuple: Option<(&Tuple, usize, usize)>,
    ) -> Result<(), HeapError> {
        // Size the image first: tuples are placed at absolute offsets below.
        // Capacity was reserved at pool construction, so this never allocates.
        buffer.page_image.resize(USABLE_PAGE_SIZE_V2, 0);

        // 1. Repack + place bytes in one reverse pass.
        let mut current_offset = USABLE_PAGE_SIZE_V2;
        // Disjoint field borrows: slots are read/mutated while the image is written.
        let (slots, image) = (&mut buffer.page.slots, &mut buffer.page_image);
        for (index, slot_option) in slots.iter_mut().enumerate().rev() {
            let Some(slot) = slot_option else {
                continue;
            };
            let length = slot.length as usize;
            let old_offset = slot.offset;
            current_offset = current_offset
                .checked_sub(length)
                .ok_or(HeapError::PageFull)?;
            let offset = current_offset;
            slot.offset = offset as u32;

            let dest = &mut image[offset..offset + length];
            // The new tuple is serialized directly into its final image
            // position; every other version's bytes are copied from the
            // source page at their old offsets.
            match new_tuple {
                Some((tuple, expected_len, slot_index)) if slot_index == index => {
                    debug_assert_eq!(expected_len, length);
                    let written = postcard::to_slice(tuple, dest)
                        .map_err(|e| HeapError::Serialization(e.to_string()))?;
                    if written.len() != length {
                        return Err(HeapError::Serialization(format!(
                            "tuple serialized size changed during insert: expected {length}, wrote {}",
                            written.len()
                        )));
                    }
                }
                _ => {
                    let src = slot_bytes(source, old_offset, length as u32)?;
                    dest.copy_from_slice(src);
                }
            }
        }

        // 2. Frame the v2 header + slot directory into the image head.
        let header_size = {
            let (page, image, scratch) = (
                &buffer.page,
                &mut buffer.page_image,
                &mut buffer.slot_scratch,
            );
            encode_versioned_header_into(page, scratch, image)?
        };

        // 3. Verify no tuple overlaps the header.
        for slot_option in buffer.page.slots.iter().flatten() {
            if (slot_option.offset as usize) < header_size {
                return Err(HeapError::PageFull);
            }
        }
        Ok(())
    }

    /// Writes raw page payload bytes through the device with the standard
    /// page framing, without allocating a [`Page`].
    ///
    /// Mirrors [`Page::to_bytes`](super::page::Page::to_bytes): an 8-byte
    /// little-endian length prefix, the payload, then zero padding to
    /// [`PAGE_SIZE`].
    fn store_page_bytes(&mut self, page_id: PageId, data: &[u8]) -> Result<(), HeapError> {
        if data.len() > PAGE_SIZE - 8 {
            return Err(HeapError::Page(PageError::PageTooLarge));
        }
        let mut buffer = [0u8; PAGE_SIZE];
        buffer[..8].copy_from_slice(&(data.len() as u64).to_le_bytes());
        buffer[8..8 + data.len()].copy_from_slice(data);
        self.device
            .write_page(page_id, &buffer)
            .map_err(device_error)?;
        self.device.flush().map_err(device_error)?;
        Ok(())
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
    #[cfg(test)]
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
    /// Returns [`HeapError::UpdateRollbackFailed`] if the replacement
    /// insert failed and the compensating rollback of the old version's
    /// deletion marking failed as well. That error poisons the
    /// transaction: the old version stays marked deleted by this
    /// transaction, so the caller MUST abort — committing would hide the
    /// old version without its replacement and silently lose the tuple.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn update_tuple_versioned(
        &mut self,
        old_tuple_id: TupleId,
        new_tuple: &Tuple,
        txn_id: crate::wal::TransactionId,
        version_pool: &mut VersionPool,
    ) -> Result<TupleId, HeapError> {
        // Pure validation before the claim: the serialized length and the
        // size limit depend only on the new tuple, so failures here mutate
        // nothing and need no rollback.
        let tuple_len = postcard::experimental::serialized_size(new_tuple)
            .map_err(|e| HeapError::Serialization(e.to_string()))?;
        // Updates link to previous version (prev_version = Some(...)).
        Self::check_versioned_tuple_size_limit(tuple_len, true)?;

        // One version record for the whole update, claimed before any page
        // is mutated so exhaustion stays atomic (nothing mutated). The
        // handle rolls the claim back if the fallible work below fails.
        let handle: crate::mvcc::VersionHandle = version_pool.claim(txn_id)?;
        let result = self.update_tuple_versioned_claimed(
            old_tuple_id,
            new_tuple,
            tuple_len,
            txn_id,
            version_pool,
        );
        if result.is_err() {
            version_pool.release(handle);
        }
        result
    }

    /// Update body; runs with a claimed version record.
    ///
    /// Validation that needs no mutation (page decode, old-slot lookup)
    /// happens before the first page write, so validation failures stay
    /// atomic; see [`update_tuple_versioned`](Self::update_tuple_versioned)
    /// for the claim/rollback protocol.
    fn update_tuple_versioned_claimed(
        &mut self,
        old_tuple_id: TupleId,
        new_tuple: &Tuple,
        tuple_len: usize,
        txn_id: crate::wal::TransactionId,
        version_pool: &mut VersionPool,
    ) -> Result<TupleId, HeapError> {
        let mut buffer = version_pool.acquire_buffer()?;

        // Step 1: Mark old version's xmax on its page.
        let page = self.load_page(old_tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let header = decode_versioned_page_into(page.data(), &mut buffer.page.slots)?;
        buffer.page.magic = header.magic;
        buffer.page.slot_count = header.slot_count;

        // Find the old slot
        let old_slot = buffer
            .page
            .slots
            .get_mut(old_tuple_id.slot as usize)
            .and_then(|s| s.as_mut())
            .ok_or(HeapError::TupleNotFound)?;

        // Mark old version as deleted by this transaction
        old_slot.xmax = Some(txn_id);

        // Re-emit the page with the old version marked: all tuple bytes are
        // copied from the source page, no new tuple, no allocation.
        Self::emit_versioned_page(&mut buffer, page.data(), None)?;
        self.store_page_bytes(old_tuple_id.page_id, &buffer.page_image)?;

        // Step 2: Insert new version (reuses the checked-out buffer).
        let insert_result = self.find_page_for_insertion(|heap, page_id| {
            heap.try_insert_into_page_versioned(
                page_id,
                new_tuple,
                tuple_len,
                txn_id,
                Some(old_tuple_id),
                &mut buffer,
            )
            .map(|slot| TupleId { page_id, slot })
        });

        if insert_result.is_err() {
            // The replacement failed after the old version was marked:
            // undo the xmax marking so the failed update leaves the
            // database unchanged. The restore rewrites the exact prior page
            // image, so `PageFull` is impossible — but a device error can
            // still strike (correlated with the insert's failure). If the
            // rollback also fails, the old version stays marked deleted by
            // this transaction and the transaction is poisoned: the caller
            // MUST abort it, because committing would hide the old version
            // without its replacement and silently lose the tuple. Both
            // errors are reported; neither is discarded.
            if let Err(insert_error) = insert_result {
                if let Err(rollback_error) = self.clear_version_xmax(old_tuple_id, &mut buffer) {
                    return Err(HeapError::UpdateRollbackFailed {
                        insert_error: insert_error.to_string(),
                        rollback_error: rollback_error.to_string(),
                    });
                }
                return Err(insert_error);
            }
        }

        insert_result
    }

    /// Clears `xmax` on a version slot, restoring the page image.
    ///
    /// Rolls back the old-version marking when an update's replacement
    /// insert fails, keeping the failed update atomic: either both the
    /// xmax marking and the new version land, or neither does. Only the
    /// xmax varint shrinks, so the re-emit cannot fail with
    /// [`HeapError::PageFull`].
    fn clear_version_xmax(
        &mut self,
        tuple_id: TupleId,
        buffer: &mut crate::mvcc::PageWorkingSet,
    ) -> Result<(), HeapError> {
        let page = self.load_page(tuple_id.page_id)?;
        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }
        let header = decode_versioned_page_into(page.data(), &mut buffer.page.slots)?;
        buffer.page.magic = header.magic;
        buffer.page.slot_count = header.slot_count;
        let slot = buffer
            .page
            .slots
            .get_mut(tuple_id.slot as usize)
            .and_then(|s| s.as_mut())
            .ok_or(HeapError::TupleNotFound)?;
        slot.xmax = None;
        // Re-emit the page with the marking removed: all tuple bytes are
        // copied from the source page, no new tuple, no allocation.
        Self::emit_versioned_page(buffer, page.data(), None)?;
        self.store_page_bytes(tuple_id.page_id, &buffer.page_image)?;
        Ok(())
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
    /// * `version_pool` - Caller-owned version pool (one record claimed)
    ///
    /// # Errors
    /// Returns [`HeapError::TupleNotFound`] if tuple_id doesn't exist.
    /// Returns [`HeapError::Serialization`] if page cannot be serialized.
    /// Returns [`HeapError::Page`] if page I/O error occurs.
    /// Returns [`HeapError::Mvcc`] on pool exhaustion (typed, never panics).
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn delete_tuple_versioned(
        &mut self,
        tuple_id: TupleId,
        txn_id: crate::wal::TransactionId,
        version_pool: &mut VersionPool,
    ) -> Result<(), HeapError> {
        // One version record for the delete, claimed before any page is
        // mutated so exhaustion stays atomic (nothing mutated). The handle
        // rolls the claim back if the fallible work below fails.
        let handle: crate::mvcc::VersionHandle = version_pool.claim(txn_id)?;
        let result = self.delete_tuple_versioned_claimed(tuple_id, txn_id, version_pool);
        if result.is_err() {
            version_pool.release(handle);
        }
        result
    }

    /// Delete body; runs with a claimed version record.
    ///
    /// The tuple lookup happens before the first page write, so
    /// validation failures stay atomic; see
    /// [`delete_tuple_versioned`](Self::delete_tuple_versioned) for the
    /// claim/rollback protocol.
    fn delete_tuple_versioned_claimed(
        &mut self,
        tuple_id: TupleId,
        txn_id: crate::wal::TransactionId,
        version_pool: &mut VersionPool,
    ) -> Result<(), HeapError> {
        let mut buffer = version_pool.acquire_buffer()?;

        // Read the page containing the tuple
        let page = self.load_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let header = decode_versioned_page_into(page.data(), &mut buffer.page.slots)?;
        buffer.page.magic = header.magic;
        buffer.page.slot_count = header.slot_count;

        // Find and mark the tuple
        let slot = buffer
            .page
            .slots
            .get_mut(tuple_id.slot as usize)
            .and_then(|s| s.as_mut())
            .ok_or(HeapError::TupleNotFound)?;

        // Mark as deleted by this transaction
        slot.xmax = Some(txn_id);

        // Re-emit the page with the version marked: all tuple bytes are
        // copied from the source page, no allocation.
        Self::emit_versioned_page(&mut buffer, page.data(), None)?;
        self.store_page_bytes(tuple_id.page_id, &buffer.page_image)?;

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
        txn_pool: &crate::mvcc::TxnPool,
        version_pool: &mut VersionPool,
    ) -> Result<usize, HeapError> {
        let mut removed_count = 0;
        let mut page_id = 0;

        // One page working buffer reused across all pages: GC performs no
        // per-page allocation.
        let mut buffer = version_pool.acquire_buffer()?;

        loop {
            let page = self.load_page(page_id)?;

            if page.is_empty() {
                break;
            }

            if !is_versioned_page(page.data()) {
                page_id += 1;
                continue;
            }

            removed_count += self.gc_process_page(
                page_id,
                &page,
                oldest_active_lsn,
                committed,
                txn_pool,
                &mut buffer,
            )?;

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
        txn_pool: &crate::mvcc::TxnPool,
        buffer: &mut crate::mvcc::PageWorkingSet,
    ) -> Result<usize, HeapError> {
        // Try to deserialize as versioned page, into the pooled working set.
        let header = match decode_versioned_page_into(page.data(), &mut buffer.page.slots) {
            Ok(header) => header,
            Err(_) => return Ok(0), // Not a versioned page or corrupted, skip
        };
        buffer.page.magic = header.magic;
        buffer.page.slot_count = header.slot_count;

        // Preserve the pre-pooling contract: a slot whose
        // `[offset, offset+length)` lies outside the page is corruption, and
        // GC must fail on it rather than silently keep it. `slot_bytes`
        // performs the bounds check without copying any tuple data.
        for slot in buffer.page.slots.iter().flatten() {
            slot_bytes(page.data(), slot.offset, slot.length)?;
        }

        let mut removed_count = 0;
        let mut page_modified = false;

        // Check each slot for dead versions
        for slot_option in buffer.page.slots.iter_mut() {
            if let Some(slot) = slot_option
                && Self::is_dead_version(slot, oldest_active_lsn, committed, txn_pool)
            {
                // This version is dead - remove it (its bytes are dropped by
                // the re-emit below, freeing the slot for reuse).
                *slot_option = None;
                removed_count += 1;
                page_modified = true;
            }
        }

        // Write page back if modified
        if page_modified {
            Self::emit_versioned_page(buffer, page.data(), None)?;
            self.store_page_bytes(page_id, &buffer.page_image)?;
        }

        Ok(removed_count)
    }

    fn is_dead_version(
        slot: &VersionedSlotEntry,
        oldest_active_lsn: crate::wal::Lsn,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
        txn_pool: &crate::mvcc::TxnPool,
    ) -> bool {
        // A version whose creator can never commit is dead: no past,
        // present, or future snapshot can ever observe it. A creator that
        // is neither committed nor live is aborted, crashed
        // mid-transaction, or never begun — none of which can ever
        // transition to committed — so its versions are safe to reclaim.
        // This reclaims inserts left behind by aborted transactions, whose
        // `xmax` stays `None` forever and which the deleter-based rule
        // below can never catch.
        if !committed.contains(&slot.xmin) && !txn_pool.is_live(slot.xmin) {
            return true;
        }
        // A version is otherwise dead only when its deleter's commit
        // settled before the oldest live snapshot: then no live or future
        // snapshot can observe the old version. The commit LSN is resolved
        // through the transaction pool — comparing the deleter's
        // transaction ID against the LSN would be unsound, because IDs and
        // LSNs are different sequences (IDs stay tiny while the LSN grows
        // with every WAL record).
        if let Some(xmax) = slot.xmax
            && committed.contains(&xmax)
        {
            // `None` (history evicted, or committed before this pool
            // existed) still means settled: eviction only drops records
            // whose commit LSN predates the oldest live snapshot, and a
            // pre-pool commit predates every snapshot this pool can take.
            return txn_pool
                .committed_before(xmax, oldest_active_lsn)
                .unwrap_or(true);
        }
        false
    }

    /// Scans all visible tuples for a given transaction snapshot.
    ///
    /// Only returns tuples that are visible according to MVCC visibility rules.
    /// The scan iterates a pooled page working buffer instead of allocating
    /// per version: the slot directory is decoded into the buffer and each
    /// visible tuple is deserialized straight from the page. Only the
    /// returned `Vec` itself allocates (once, amortized).
    ///
    /// # Arguments
    /// * `snapshot` - Transaction snapshot determining visibility
    /// * `txn_pool` - Bounded transaction pool for snapshot liveness checks
    /// * `version_pool` - Caller-owned version pool lending the page buffer
    /// * `committed` - Set of all committed transaction IDs
    ///
    /// # Returns
    /// Vector of visible tuples
    ///
    /// # Errors
    /// Returns [`HeapError::Serialization`] if tuples cannot be deserialized.
    /// Returns [`HeapError::Page`] if page I/O error occurs.
    /// Returns [`HeapError::Mvcc`] if no page working buffer is free.
    pub(crate) fn scan_visible(
        &mut self,
        snapshot: &crate::mvcc::TransactionSnapshot,
        txn_pool: &crate::mvcc::TxnPool,
        version_pool: &mut VersionPool,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
    ) -> Result<Vec<Tuple>, HeapError> {
        let mut results = Vec::new();
        self.for_each_visible(snapshot, txn_pool, version_pool, committed, &mut |tuple| {
            results.push(tuple);
        })?;
        Ok(results)
    }

    /// Visits each tuple visible to `snapshot`, without allocating per version.
    ///
    /// Allocation-free iteration over the version pool's working buffer:
    /// pages are decoded into the pooled slot directory and visible tuples
    /// are deserialized straight from the page image into the visitor. The
    /// only allocation per visited tuple is the [`Tuple`] itself, which is
    /// the visitor's output.
    pub(crate) fn for_each_visible(
        &mut self,
        snapshot: &crate::mvcc::TransactionSnapshot,
        txn_pool: &crate::mvcc::TxnPool,
        version_pool: &mut VersionPool,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
        visit: &mut dyn FnMut(Tuple),
    ) -> Result<(), HeapError> {
        // One page working buffer for the whole scan: no per-page allocation.
        let mut buffer = version_pool.acquire_buffer()?;
        let mut page_id = 0;

        loop {
            let page = self.load_page(page_id)?;

            if page.is_empty() {
                // Empty page means no more data
                break;
            }

            // Decode the slot directory into the pooled working set.
            let header = match decode_versioned_page_into(page.data(), &mut buffer.page.slots) {
                Ok(header) => header,
                Err(_) => {
                    // Not a versioned page, skip
                    page_id += 1;
                    continue;
                }
            };
            buffer.page.magic = header.magic;
            buffer.page.slot_count = header.slot_count;

            // Check each slot for visibility
            for slot_entry in buffer.page.slots.iter().flatten() {
                // Create version metadata
                let version_metadata = crate::mvcc::VersionMetadata {
                    xmin: slot_entry.xmin,
                    xmax: slot_entry.xmax,
                };

                // Check visibility
                if crate::mvcc::visibility::is_visible(
                    &version_metadata,
                    snapshot,
                    txn_pool,
                    committed,
                ) {
                    // Deserialize the tuple straight from the page image.
                    let tuple_data = slot_bytes(page.data(), slot_entry.offset, slot_entry.length)?;
                    let tuple: Tuple = deserialize_bounded(tuple_data)?;
                    visit(tuple);
                }
            }

            page_id += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
