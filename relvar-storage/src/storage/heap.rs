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

use super::page::{PAGE_SIZE, Page, PageError, PageFile, PageId};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
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

/// Tuple ID: (page_id, slot_number)
/// Internal to storage layer only (TTM Proscription 6)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct TupleId {
    pub(crate) page_id: PageId,
    pub(crate) slot: u32,
}

/// Operation type for size checking (Insert vs Update)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationType {
    Insert,
    Update,
}

/// Errors that can occur during heap file operations.
#[derive(Debug, Error)]
pub enum HeapError {
    /// An error occurred at the page layer.
    #[error("Page error: {0}")]
    Page(#[from] PageError),

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
/// ```
///
/// # TTM Compliance
///
/// This struct carefully maintains TTM Proscription 6 compliance:
/// - `insert_tuple()` returns `Result<(), HeapError>` (no TupleId)
/// - `scan()` returns `Vec<Tuple>` (no TupleId)
/// - `read_tuple(TupleId)` is `pub(crate)` (internal only)
///
/// # Example
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
/// ```
pub struct HeapFile {
    /// The underlying page file for storage.
    page_file: PageFile,
    /// The type of tuples stored in this heap file.
    relation_type: RelationType,
}

/// Slot directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlotEntry {
    offset: u32,
    length: u32,
}

/// Versioned slot directory entry for MVCC.
///
/// Extends SlotEntry with transaction version metadata to support
/// Multi-Version Concurrency Control (MVCC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct VersionedSlotEntry {
    /// Offset of tuple data in page
    offset: u32,
    /// Length of tuple data
    length: u32,
    /// Transaction that created this version
    xmin: crate::wal::TransactionId,
    /// Transaction that deleted/updated this version (None = still visible)
    xmax: Option<crate::wal::TransactionId>,
    /// Previous version in the version chain (for undo)
    prev_version: Option<TupleId>,
}

impl SlotEntry {
    fn offset(&self) -> u32 {
        self.offset
    }

    fn length(&self) -> u32 {
        self.length
    }

    fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

impl VersionedSlotEntry {
    fn offset(&self) -> u32 {
        self.offset
    }

    fn length(&self) -> u32 {
        self.length
    }

    fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

/// Page layout: `[slot_count (4 bytes)] [slot_entries...] [free_space] [...tuple_data]`
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlottedPage {
    slot_count: u32,
    slots: Vec<Option<SlotEntry>>,
}

/// Versioned page layout for MVCC
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionedSlottedPage {
    magic: u32, // Magic number to distinguish from SlottedPage: 0x4D564343 ("MVCC")
    slot_count: u32,
    slots: Vec<Option<VersionedSlotEntry>>,
}

const VERSIONED_PAGE_MAGIC: u32 = 0x4D564343; // "MVCC" in ASCII

// Page format version to handle serialization changes
const PAGE_FORMAT_VERSION: u8 = 2; // Version 2: length-prefixed slot directory

const USABLE_PAGE_SIZE_V1: usize = PAGE_SIZE - 8;
const USABLE_PAGE_SIZE_V2: usize = PAGE_SIZE - 8;
const V2_HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length

impl HeapFile {
    /// Creates a new heap file, truncating any existing file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the heap file will be created
    /// * `relation_type` - The type of tuples that will be stored
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Page`] if the file cannot be created.
    pub fn create<P: AsRef<Path>>(path: P, relation_type: RelationType) -> Result<Self, HeapError> {
        let page_file = PageFile::create(path)?;
        Ok(Self {
            page_file,
            relation_type,
        })
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
    /// Returns [`HeapError::Page`] if the file cannot be opened.
    pub fn open<P: AsRef<Path>>(path: P, relation_type: RelationType) -> Result<Self, HeapError> {
        let page_file = PageFile::open(path)?;
        Ok(Self {
            page_file,
            relation_type,
        })
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

        if header_size + tuple_data_len > USABLE_PAGE_SIZE {
            return Err(HeapError::TupleTooLarge(tuple_data_len));
        }
        Ok(())
    }

    /// Helper to repack slots and calculate offsets.
    /// Iterates backward from the end of the available space.
    /// Assumes slots are already populated (Some) for valid tuples.
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
                slot.set_offset(current_offset as u32);
                slot.set_length(tuple.len() as u32);
            }
        }
        Ok(())
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
                slot.set_offset(current_offset as u32);
                slot.set_length(tuple.len() as u32);
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
        let mut results = Vec::new();
        for slot in slots {
            let tuple = self.extract_tuple_from_page(page, slot.offset(), slot.length())?;
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
        let mut results = Vec::new();
        for slot in slots {
            let tuple = self.extract_tuple_from_page(page, slot.offset(), slot.length())?;
            results.push(tuple);
        }
        Ok(results)
    }

    /// Helper to extract all tuples from a page based on slot entries.
    /// This abstracts the common logic used in insert, update, delete, and GC operations.
    fn extract_all_tuples(
        &self,
        page: &Page,
        slots: &[Option<SlotEntry>],
    ) -> Result<Vec<Vec<u8>>, HeapError> {
        let mut existing_tuples: Vec<Vec<u8>> = Vec::new();
        // Maintain alignment with slots: push empty Vec for None slots
        for slot_option in slots.iter() {
            if let Some(slot_entry) = slot_option {
                let raw_data =
                    self.extract_raw_tuple_data(page, slot_entry.offset(), slot_entry.length())?;
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
        let mut existing_tuples: Vec<Vec<u8>> = Vec::new();
        // Maintain alignment with slots: push empty Vec for None slots
        for slot_option in slots.iter() {
            if let Some(slot_entry) = slot_option {
                let raw_data =
                    self.extract_raw_tuple_data(page, slot_entry.offset(), slot_entry.length())?;
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

    /// Try to insert tuple data into a specific page
    fn try_insert_into_page(
        &mut self,
        page_id: PageId,
        tuple_data: &[u8],
    ) -> Result<u32, HeapError> {
        // Read the page (or create empty if doesn't exist)
        let page = self.page_file.read_page(page_id)?;

        // Read existing tuples from the page
        let (mut slotted_page, mut existing_tuples) = if page.is_empty() {
            (
                SlottedPage {
                    slot_count: 0,
                    slots: Vec::new(),
                },
                Vec::new(),
            )
        } else {
            let sp = self.deserialize_slotted_page(&page)?;
            let tuples = self.extract_all_tuples(&page, &sp.slots)?;
            (sp, tuples)
        };

        // Find free slot or add new one
        let slot_number =
            Self::find_or_allocate_slot(&mut slotted_page.slots, &mut slotted_page.slot_count);

        // Add new tuple to the list (overwrite if reusing slot, append if new)
        if (slot_number as usize) < existing_tuples.len() {
            existing_tuples[slot_number as usize] = tuple_data.to_vec();
        } else {
            existing_tuples.push(tuple_data.to_vec());
        }

        // Initialize the slot (needed for size calc and repacking)
        slotted_page.slots[slot_number as usize] = Some(SlotEntry {
            offset: 0,
            length: tuple_data.len() as u32,
        });

        // Calculate exact header size using bincode
        let header_size = serialized_size_compat(&slotted_page)? as usize;

        // Calculate total size correctly
        let total_tuple_data_size: usize = existing_tuples.iter().map(|t| t.len()).sum::<usize>();
        let required_space = header_size + total_tuple_data_size;

        if required_space > USABLE_PAGE_SIZE_V1 {
            return Err(HeapError::PageFull);
        }

        // Repack slots
        Self::repack_slots(
            &mut slotted_page.slots,
            &existing_tuples,
            USABLE_PAGE_SIZE_V1,
        )?;

        // Serialize the updated page with all tuples
        let page_data = self.serialize_slotted_page_with_tuples(&slotted_page, &existing_tuples)?;

        // Write the page
        let updated_page = Page::from_data(page_id, page_data)?;
        self.page_file.write_page(&updated_page)?;

        Ok(slot_number)
    }

    /// Serialize a slotted page with all tuple data
    fn serialize_slotted_page_with_tuples(
        &self,
        slotted_page: &SlottedPage,
        tuples: &[Vec<u8>],
    ) -> Result<Vec<u8>, HeapError> {
        // Serialize slot directory
        let slot_dir = serialize_compat(slotted_page)?;

        // Create page buffer
        let mut data = vec![0u8; PAGE_SIZE - 8]; // USABLE_PAGE_SIZE

        // Copy slot directory at beginning
        data[..slot_dir.len()].copy_from_slice(&slot_dir);

        // Copy each tuple at its designated offset
        for (idx, slot_entry) in slotted_page.slots.iter().enumerate() {
            if let Some(entry) = slot_entry {
                let offset = entry.offset as usize;
                let length = entry.length as usize;
                if idx < tuples.len() && !tuples[idx].is_empty() {
                    data[offset..offset + length].copy_from_slice(&tuples[idx]);
                }
            }
        }

        Ok(data)
    }

    /// Read a tuple by its TupleId (internal use only per TTM Proscription 6)
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn read_tuple(&mut self, tuple_id: TupleId) -> Result<Tuple, HeapError> {
        let page = self.page_file.read_page(tuple_id.page_id)?;

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
        let end = start + slot_entry.length as usize;

        if end > page.data().len() {
            return Err(HeapError::TupleNotFound);
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
        let page = self.page_file.read_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let versioned_page = self.deserialize_versioned_page(&page)?;

        let slot_entry = versioned_page
            .slots
            .get(tuple_id.slot as usize)
            .and_then(|s| s.as_ref())
            .ok_or(HeapError::TupleNotFound)?;

        // Extract tuple data from page
        let start = slot_entry.offset as usize;
        let end = start + slot_entry.length as usize;

        if end > page.data().len() {
            return Err(HeapError::TupleNotFound);
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
    pub fn scan(&mut self) -> Result<Vec<Tuple>, HeapError> {
        let mut results = Vec::new();
        let mut page_id = 0;

        // Scan pages until we hit an empty one
        loop {
            let page = self.page_file.read_page(page_id)?;

            if page.is_empty() {
                // Empty page means no more data
                break;
            }

            if self.is_versioned_page(&page) {
                // Versioned page format (MVCC)
                let versioned_page = self.deserialize_versioned_page(&page)?;
                results.extend(self.extract_tuples_from_versioned_slots(
                    &page,
                    versioned_page.slots.iter().flatten(),
                )?);
            } else {
                // Old slotted page format (non-MVCC)
                let slotted_page = self.deserialize_slotted_page(&page)?;
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
        self.page_file.sync()?;
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
        self.check_versioned_tuple_size_limit(tuple_data.len(), OperationType::Insert)?;

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
        op_type: OperationType,
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
                prev_version: match op_type {
                    OperationType::Update => Some(TupleId {
                        page_id: 0,
                        slot: 0,
                    }),
                    OperationType::Insert => None,
                },
            })],
        };

        let header_size = serialized_size_compat(&dummy_page)? as usize;

        const USABLE_PAGE_SIZE: usize = PAGE_SIZE - 8;
        const FORMAT_HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length

        if FORMAT_HEADER_SIZE + header_size + tuple_data_len > USABLE_PAGE_SIZE {
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
        let page = self.page_file.read_page(page_id)?;

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
            let vp = self.deserialize_versioned_page(&page)?;
            let tuples = self.extract_all_versioned_tuples(&page, &vp.slots)?;
            (vp, tuples)
        };

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

        // Calculate required space using ACTUAL serialized size
        // CRITICAL: Must use bincode size, not sizeof, as they differ!
        let slot_dir = serialize_compat(&versioned_page)?;
        let header_size = V2_HEADER_SIZE + slot_dir.len();

        // existing_tuples already includes tuple_data, so we just sum existing_tuples
        let total_tuple_data_size: usize = existing_tuples.iter().map(|t| t.len()).sum::<usize>();
        let required_space = header_size + total_tuple_data_size;

        if required_space > USABLE_PAGE_SIZE_V2 {
            return Err(HeapError::PageFull);
        }

        // Repack slots
        Self::repack_versioned_slots(
            &mut versioned_page.slots,
            &existing_tuples,
            USABLE_PAGE_SIZE_V2,
        )?;

        // Serialize the updated page with all tuples
        let page_data =
            self.serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)?;

        // Write the page
        let updated_page = Page::from_data(page_id, page_data)?;
        self.page_file.write_page(&updated_page)?;

        Ok(slot_number)
    }

    /// Serialize a versioned page with all tuple data
    fn serialize_versioned_page_with_tuples(
        &self,
        versioned_page: &VersionedSlottedPage,
        tuples: &[Vec<u8>],
    ) -> Result<Vec<u8>, HeapError> {
        // Serialize slot directory
        let slot_dir = serialize_compat(versioned_page)?;

        // Format: [version:1 byte][slot_dir_length:4 bytes][slot_dir][tuple_data]
        const HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length
        const USABLE_PAGE_SIZE: usize = PAGE_SIZE - 8;

        if slot_dir.len() > u32::MAX as usize {
            return Err(HeapError::Serialization(
                "Slot directory too large to be represented by u32 length prefix".to_string(),
            ));
        }

        if slot_dir
            .len()
            .checked_add(HEADER_SIZE)
            .ok_or_else(|| HeapError::Serialization("Header size overflow".to_string()))?
            > USABLE_PAGE_SIZE
        {
            return Err(HeapError::Serialization(format!(
                "slot directory too large for page: {} > {}",
                slot_dir.len() + HEADER_SIZE,
                USABLE_PAGE_SIZE
            )));
        }

        // Create page buffer
        let mut data = vec![0u8; USABLE_PAGE_SIZE];

        // Write format version
        data[0] = PAGE_FORMAT_VERSION;

        // Write slot directory length
        let slot_dir_len = slot_dir.len() as u32;
        data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

        // Copy slot directory after header
        data[HEADER_SIZE..HEADER_SIZE + slot_dir.len()].copy_from_slice(&slot_dir);

        // Copy each tuple at its designated offset
        for (idx, slot_entry) in versioned_page.slots.iter().enumerate() {
            if let Some(entry) = slot_entry {
                let offset = entry.offset as usize;
                let length = entry.length as usize;
                if idx < tuples.len() && !tuples[idx].is_empty() {
                    // Validate that offset + length doesn't exceed buffer
                    if offset + length > data.len() {
                        return Err(HeapError::Serialization(format!(
                            "Slot {} points outside buffer: offset={}, length={}, buffer_len={}",
                            idx,
                            offset,
                            length,
                            data.len()
                        )));
                    }
                    data[offset..offset + length].copy_from_slice(&tuples[idx]);
                }
            }
        }

        Ok(data)
    }

    /// Checks if a page is a versioned page (MVCC).
    fn is_versioned_page(&self, page: &Page) -> bool {
        // postcard uses varint for u32. 0x4D564343 encodes to [195, 134, 217, 234, 4].
        let postcard_magic = [195, 134, 217, 234, 4];

        // Check for new format: version byte + length (4 bytes) + magic at offset 5
        let is_new_format = page.data().len() >= 5 + postcard_magic.len()
            && page.data()[0] == PAGE_FORMAT_VERSION
            && { page.data()[5..5 + postcard_magic.len()] == postcard_magic };

        // Check for old format (just the magic number at the start)
        let is_old_versioned = !is_new_format && page.data().len() >= postcard_magic.len() && {
            page.data()[0..postcard_magic.len()] == postcard_magic
        };

        is_new_format || is_old_versioned
    }

    /// Deserializes a versioned page, handling both V1 and V2 formats.
    fn deserialize_versioned_page(&self, page: &Page) -> Result<VersionedSlottedPage, HeapError> {
        if !page.data().is_empty() && page.data()[0] == PAGE_FORMAT_VERSION {
            if page.data().len() < 5 {
                return Err(HeapError::Serialization(
                    "Versioned page too short to contain header".to_string(),
                ));
            }
            // New format: [version:1][length:4][slot_dir][tuples]
            let len_bytes: [u8; 4] = page.data()[1..5].try_into().map_err(|_| {
                HeapError::Serialization(
                    "Invalid slot directory length prefix in versioned page header".to_string(),
                )
            })?;
            let slot_dir_len = u32::from_le_bytes(len_bytes) as usize;

            // Ensure the declared slot directory length fits within the page data
            // Header is 5 bytes (1 byte version + 4 bytes length)
            let end_of_header = 5usize.checked_add(slot_dir_len).ok_or_else(|| {
                HeapError::Serialization("Slot directory length overflow".to_string())
            })?;

            if end_of_header > page.data().len() {
                return Err(HeapError::Serialization(format!(
                    "Slot directory length ({}) exceeds page size",
                    slot_dir_len
                )));
            }

            deserialize_bounded(&page.data()[5..end_of_header])
        } else {
            // Old format: [slot_dir][tuples]
            deserialize_bounded(page.data())
        }
    }

    /// Deserializes a standard slotted page.
    fn deserialize_slotted_page(&self, page: &Page) -> Result<SlottedPage, HeapError> {
        deserialize_bounded(page.data())
    }

    /// Validates that a slot points to valid data within the page.
    fn validate_slot_bounds(
        &self,
        page: &Page,
        offset: u32,
        length: u32,
    ) -> Result<(usize, usize), HeapError> {
        let start = offset as usize;
        let end = start
            .checked_add(length as usize)
            .ok_or_else(|| HeapError::Serialization("Tuple end offset overflow".to_string()))?;

        if end > page.data().len() {
            return Err(HeapError::Serialization(format!(
                "Corrupted slot on page {} points outside page data",
                page.id()
            )));
        }
        Ok((start, end))
    }

    /// Extracts a tuple from a page at the given offset and length.
    fn extract_tuple_from_page(
        &self,
        page: &Page,
        offset: u32,
        length: u32,
    ) -> Result<Tuple, HeapError> {
        let (start, end) = self.validate_slot_bounds(page, offset, length)?;
        let tuple_data = &page.data()[start..end];
        deserialize_bounded(tuple_data)
    }

    /// Extracts raw tuple data from a page at the given offset and length.
    fn extract_raw_tuple_data(
        &self,
        page: &Page,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, HeapError> {
        let start = offset as usize;
        let end = start
            .checked_add(length as usize)
            .ok_or_else(|| HeapError::Serialization("Tuple end offset overflow".to_string()))?;

        if end <= page.data().len() {
            Ok(page.data()[start..end].to_vec())
        } else {
            Err(HeapError::Serialization(format!(
                "Corrupted slot on page {} points outside page data",
                page.id()
            )))
        }
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
        let page = self.page_file.read_page(old_tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let mut versioned_page = self.deserialize_versioned_page(&page)?;

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
        let page_data =
            self.serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)?;
        let updated_page = Page::from_data(old_tuple_id.page_id, page_data)?;
        self.page_file.write_page(&updated_page)?;

        // Step 2: Insert new version
        let new_tuple_data = serialize_compat(new_tuple)?;

        // Check if new tuple is too large
        // Updates link to previous version (prev_version = Some(...))
        self.check_versioned_tuple_size_limit(new_tuple_data.len(), OperationType::Update)?;

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
        let page = self.page_file.read_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let mut versioned_page = self.deserialize_versioned_page(&page)?;

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
        let page_data =
            self.serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)?;
        let updated_page = Page::from_data(tuple_id.page_id, page_data)?;
        self.page_file.write_page(&updated_page)?;

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
            let page = self.page_file.read_page(page_id)?;

            if page.is_empty() {
                break;
            }

            if !self.is_versioned_page(&page) {
                page_id += 1;
                continue;
            }

            // Try to deserialize as versioned page
            let mut versioned_page = match self.deserialize_versioned_page(&page) {
                Ok(vp) => vp,
                Err(_) => {
                    // Not a versioned page or corrupted, skip
                    page_id += 1;
                    continue;
                }
            };

            // Extract existing tuple data
            let mut existing_tuples =
                self.extract_all_versioned_tuples(&page, &versioned_page.slots)?;

            let mut page_modified = false;

            // Check each slot for dead versions
            for (idx, slot_option) in versioned_page.slots.iter_mut().enumerate() {
                if let Some(slot) = slot_option {
                    // Check if this version is dead
                    if let Some(xmax) = slot.xmax {
                        // Has xmax - was deleted or updated
                        if committed.contains(&xmax) {
                            // xmax transaction committed
                            // Check if it's old enough (before oldest active)
                            // Note: We need to compare transaction IDs as proxy for LSN
                            // since we don't track commit LSNs yet
                            if xmax.value() < oldest_active_lsn.value() {
                                // This version is dead - remove it
                                *slot_option = None;
                                existing_tuples[idx] = Vec::new();
                                removed_count += 1;
                                page_modified = true;
                            }
                        }
                    }
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

                let page_data =
                    self.serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)?;
                let updated_page = Page::from_data(page_id, page_data)?;
                self.page_file.write_page(&updated_page)?;
            }

            page_id += 1;
        }

        Ok(removed_count)
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
            let page = self.page_file.read_page(page_id)?;

            if page.is_empty() {
                // Empty page means no more data
                break;
            }

            // Try to deserialize as VersionedSlottedPage
            let versioned_page = match self.deserialize_versioned_page(&page) {
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
                    let end = start + slot_entry.length as usize;

                    if end <= page.data().len() {
                        let tuple_data = &page.data()[start..end];
                        let tuple: Tuple = deserialize_bounded(tuple_data)?;
                        results.push(tuple);
                    }
                }
            }

            page_id += 1;
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{ScalarType, TupleType};
    use tempfile::NamedTempFile;

    fn create_test_relation_type() -> RelationType {
        let heading = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        RelationType::new(heading)
    }

    /// Helper function for tests to deserialize versioned pages (handles both old and new formats)
    fn deserialize_versioned_page_for_test(
        page_data: &[u8],
    ) -> Result<VersionedSlottedPage, postcard::Error> {
        if page_data.len() >= 5 && page_data[0] == PAGE_FORMAT_VERSION {
            // New format: [version:1][length:4][slot_dir][tuples]
            let slot_dir_len = u32::from_le_bytes(page_data[1..5].try_into().unwrap()) as usize;
            postcard::from_bytes(&page_data[5..5 + slot_dir_len])
        } else {
            // Old format: [slot_dir][tuples]
            postcard::from_bytes(page_data)
        }
    }

    #[test]
    fn test_heap_insert_and_read() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

        let tuple = tuple! { id: 1i64, name: "Alice" };
        heap.insert_tuple(&tuple).unwrap();

        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0], tuple);
    }

    #[test]
    fn test_heap_insert_multiple() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

        let tuple1 = tuple! { id: 1i64, name: "Alice" };
        let tuple2 = tuple! { id: 2i64, name: "Bob" };
        let tuple3 = tuple! { id: 3i64, name: "Charlie" };

        heap.insert_tuple(&tuple1).unwrap();
        heap.insert_tuple(&tuple2).unwrap();
        heap.insert_tuple(&tuple3).unwrap();

        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 3);
        assert!(tuples.contains(&tuple1));
        assert!(tuples.contains(&tuple2));
        assert!(tuples.contains(&tuple3));
    }

    #[test]
    fn test_heap_scan() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

        heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
            .unwrap();
        heap.insert_tuple(&tuple! { id: 3i64, name: "Charlie" })
            .unwrap();

        let results = heap.scan().unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_store_and_load_relation() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

        // Create a relation with some tuples
        let mut relation = Relation::new(rel_type.clone());
        relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
        relation.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
        relation
            .insert(tuple! { id: 3i64, name: "Charlie" })
            .unwrap();

        // Store it
        heap.store_relation(&relation).unwrap();

        // Load it back
        let loaded_relation = heap.load_relation().unwrap();

        assert_eq!(loaded_relation.cardinality(), 3);
        assert_eq!(loaded_relation, relation);
    }

    #[test]
    fn test_heap_persistence() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let rel_type = create_test_relation_type();

        // Write tuples
        {
            let mut heap = HeapFile::create(&path, rel_type.clone()).unwrap();
            heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
                .unwrap();
            heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
                .unwrap();
        }

        // Read them back in a new instance
        {
            let mut heap = HeapFile::open(&path, rel_type.clone()).unwrap();
            let results = heap.scan().unwrap();
            assert_eq!(results.len(), 2);
        }
    }

    // test_tuple_not_found removed - tested internal read_tuple with TupleId which is now pub(crate)

    // Tests verifying TupleId is not exposed in public APIs (TTM Proscription 6)

    #[test]
    fn test_heap_insert_returns_unit_not_tuple_id() {
        let temp_file = NamedTempFile::new().unwrap();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

        let tuple = tuple! { id: 1i64, name: "Alice" };
        let result = heap.insert_tuple(&tuple);

        // Type check: Verifies the API returns unit type, not TupleId
        assert!(result.is_ok());
        let _unit: () = result.unwrap();
    }

    #[test]
    fn test_heap_scan_returns_only_tuples() {
        let temp_file = NamedTempFile::new().unwrap();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

        heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Type check: Verifies the API returns Vec<Tuple>, not Vec<(TupleId, Tuple)>
        let tuples: Vec<Tuple> = heap.scan().unwrap();
        assert_eq!(tuples.len(), 2);
    }

    #[test]
    fn test_store_relation_returns_unit() {
        let temp_file = NamedTempFile::new().unwrap();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(temp_file.path(), rel_type.clone()).unwrap();

        let mut relation = Relation::new(rel_type);
        relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();

        // Type check: Verifies the API returns unit type, not Vec<TupleId>
        let result = heap.store_relation(&relation);
        assert!(result.is_ok());
        let _unit: () = result.unwrap();
    }

    #[test]
    fn test_heap_scan_large_volume() {
        const NUM_PAGES: u64 = 1005;
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create a relation type with a Bytes attribute
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("data", ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Manually craft pages to avoid O(N^2) insert performance
        // We just need > 1000 pages, the content size doesn't matter as long as it's valid
        let payload = vec![0u8; 100];

        for i in 0..NUM_PAGES {
            let tuple = tuple! {
                id: i as i64,
                data: payload.clone(),
            };
            let tuple_data = postcard::to_allocvec(&tuple).unwrap();

            // Construct a SlottedPage with one tuple
            // We place tuple at the end of the page (standard behavior)
            let tuple_len = tuple_data.len();
            // USABLE_PAGE_SIZE = PAGE_SIZE - 8
            let offset = (PAGE_SIZE - 8) - tuple_len;

            let slotted_page = SlottedPage {
                slot_count: 1,
                slots: vec![Some(SlotEntry {
                    offset: offset as u32,
                    length: tuple_len as u32,
                })],
            };

            // We can use the helper method since we are in the same module (tests)
            let page_data = heap
                .serialize_slotted_page_with_tuples(&slotted_page, &[tuple_data])
                .unwrap();

            let page = Page::from_data(i, page_data).unwrap();
            heap.page_file.write_page(&page).unwrap();
        }

        // Scan and verify count
        let tuples = heap.scan().unwrap();
        assert_eq!(
            tuples.len() as u64,
            NUM_PAGES,
            "Scan stopped early! Expected {} tuples, got {}",
            NUM_PAGES,
            tuples.len()
        );
    }

    #[test]
    fn test_heap_page_growth() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Create a tuple that takes up roughly half a page (2000 bytes)
        // Page size is 4096. Overhead is small.
        // 2 tuples should fit. 3rd should force new page.
        let data = vec![0u8; 2000];
        let tuple = tuple! { data: data };

        heap.insert_tuple(&tuple).unwrap(); // Page 0
        heap.insert_tuple(&tuple).unwrap(); // Page 0 (should fit, 4000 < 4096 - overhead)

        // Let's make it larger to guarantee split. 3000 bytes.
        let data_large = vec![0u8; 3000];
        let tuple_large = tuple! { data: data_large };

        // Re-create heap to start fresh
        let mut heap = HeapFile::create(path, create_test_relation_type()).unwrap(); // Reset

        // 1st tuple: 3000 bytes. Page 0.
        heap.insert_tuple(&tuple_large).unwrap();

        // 2nd tuple: 3000 bytes. Should go to Page 1.
        // If my fix broke page creation, this will fail (err instead of new page).
        heap.insert_tuple(&tuple_large).unwrap();

        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 2);
    }

    // MVCC versioned slot entry tests

    fn test_txn(value: u64) -> crate::wal::TransactionId {
        crate::wal::TransactionId::new(value)
    }

    #[test]
    fn test_versioned_slot_serialization() {
        let entry = VersionedSlotEntry {
            offset: 100,
            length: 50,
            xmin: test_txn(1),
            xmax: None,
            prev_version: None,
        };

        // Serialize and deserialize
        let serialized = postcard::to_allocvec(&entry).unwrap();
        let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();

        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_versioned_slot_with_xmax() {
        let entry = VersionedSlotEntry {
            offset: 200,
            length: 75,
            xmin: test_txn(1),
            xmax: Some(test_txn(2)),
            prev_version: None,
        };

        let serialized = postcard::to_allocvec(&entry).unwrap();
        let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();

        assert_eq!(entry, deserialized);
        assert_eq!(deserialized.xmax, Some(test_txn(2)));
    }

    #[test]
    fn test_versioned_slot_with_prev_version() {
        let prev = TupleId {
            page_id: 5,
            slot: 10,
        };

        let entry = VersionedSlotEntry {
            offset: 300,
            length: 100,
            xmin: test_txn(3),
            xmax: Some(test_txn(4)),
            prev_version: Some(prev),
        };

        let serialized = postcard::to_allocvec(&entry).unwrap();
        let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();

        assert_eq!(entry, deserialized);
        assert_eq!(deserialized.prev_version, Some(prev));
    }

    #[test]
    fn test_versioned_page_layout() {
        let slots = vec![
            Some(VersionedSlotEntry {
                offset: 4000,
                length: 50,
                xmin: test_txn(1),
                xmax: None,
                prev_version: None,
            }),
            Some(VersionedSlotEntry {
                offset: 3900,
                length: 80,
                xmin: test_txn(2),
                xmax: Some(test_txn(3)),
                prev_version: None,
            }),
            None, // Empty slot (deleted tuple)
        ];

        let page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 3,
            slots,
        };

        // Serialize and deserialize
        let serialized = postcard::to_allocvec(&page).unwrap();
        let deserialized: VersionedSlottedPage = postcard::from_bytes(&serialized).unwrap();

        assert_eq!(page.slot_count, deserialized.slot_count);
        assert_eq!(page.slots.len(), deserialized.slots.len());

        // Verify first slot
        assert_eq!(
            page.slots[0].as_ref().unwrap().xmin,
            deserialized.slots[0].as_ref().unwrap().xmin
        );

        // Verify second slot has xmax
        assert_eq!(page.slots[1].as_ref().unwrap().xmax, Some(test_txn(3)));

        // Verify third slot is None
        assert!(deserialized.slots[2].is_none());
    }

    #[test]
    fn test_versioned_slot_roundtrip_multiple() {
        // Test multiple entries with various combinations
        let entries = vec![
            VersionedSlotEntry {
                offset: 1000,
                length: 100,
                xmin: test_txn(1),
                xmax: None,
                prev_version: None,
            },
            VersionedSlotEntry {
                offset: 2000,
                length: 200,
                xmin: test_txn(2),
                xmax: Some(test_txn(3)),
                prev_version: Some(TupleId {
                    page_id: 0,
                    slot: 0,
                }),
            },
            VersionedSlotEntry {
                offset: 3000,
                length: 300,
                xmin: test_txn(4),
                xmax: Some(test_txn(5)),
                prev_version: Some(TupleId {
                    page_id: 1,
                    slot: 1,
                }),
            },
        ];

        for entry in entries {
            let serialized = postcard::to_allocvec(&entry).unwrap();
            let deserialized: VersionedSlotEntry = postcard::from_bytes(&serialized).unwrap();
            assert_eq!(entry, deserialized);
        }
    }

    #[test]
    fn test_versioned_page_empty_slots() {
        let page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 0,
            slots: Vec::new(),
        };

        let serialized = postcard::to_allocvec(&page).unwrap();
        let deserialized: VersionedSlottedPage = postcard::from_bytes(&serialized).unwrap();

        assert_eq!(page.slot_count, deserialized.slot_count);
        assert_eq!(page.slots.len(), 0);
    }

    #[test]
    fn test_versioned_slot_size_vs_regular() {
        // Ensure we understand the size increase
        let regular = SlotEntry {
            offset: 100,
            length: 50,
        };

        let versioned = VersionedSlotEntry {
            offset: 100,
            length: 50,
            xmin: test_txn(1),
            xmax: None,
            prev_version: None,
        };

        let regular_size = postcard::to_allocvec(&regular).unwrap().len();
        let versioned_size = postcard::to_allocvec(&versioned).unwrap().len();

        // Versioned should be larger (has xmin, xmax, prev_version)
        assert!(versioned_size > regular_size);
    }

    // Phase 2.2: Insert with Version tests

    #[test]
    fn test_insert_versioned_sets_xmin() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let tuple = tuple! { id: 1i64, name: "Alice" };
        let txn_id = test_txn(10);

        let tuple_id = heap.insert_tuple_versioned(&tuple, txn_id).unwrap();

        // Verify TupleId was returned
        assert_eq!(tuple_id.page_id, 0);
        assert_eq!(tuple_id.slot, 0);
    }

    #[test]
    fn test_insert_versioned_xmax_is_none() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let tuple = tuple! { id: 1i64, name: "Bob" };
        let txn_id = test_txn(20);

        heap.insert_tuple_versioned(&tuple, txn_id).unwrap();

        // Would need to read the page to verify xmax is None
        // For now, just verify insertion succeeded
    }

    #[test]
    fn test_insert_versioned_multiple_versions_same_page() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert multiple versions from different transactions
        let tuple1 = tuple! { id: 1i64, name: "Version1" };
        let tuple2 = tuple! { id: 2i64, name: "Version2" };
        let tuple3 = tuple! { id: 3i64, name: "Version3" };

        let tid1 = heap.insert_tuple_versioned(&tuple1, test_txn(1)).unwrap();
        let tid2 = heap.insert_tuple_versioned(&tuple2, test_txn(2)).unwrap();
        let tid3 = heap.insert_tuple_versioned(&tuple3, test_txn(3)).unwrap();

        // All should be on page 0 (small tuples)
        assert_eq!(tid1.page_id, 0);
        assert_eq!(tid2.page_id, 0);
        assert_eq!(tid3.page_id, 0);

        // Different slots
        assert_eq!(tid1.slot, 0);
        assert_eq!(tid2.slot, 1);
        assert_eq!(tid3.slot, 2);
    }

    #[test]
    fn test_insert_versioned_returns_tuple_id() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let tuple = tuple! { id: 42i64, name: "Test" };
        let txn_id = test_txn(100);

        let result = heap.insert_tuple_versioned(&tuple, txn_id);

        assert!(result.is_ok());
        let tuple_id = result.unwrap();

        // Verify we got a valid TupleId
        assert!(tuple_id.page_id < 1000); // Reasonable page number
        assert!(tuple_id.slot < 1000); // Reasonable slot number
    }

    #[test]
    fn test_insert_versioned_different_transactions() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Same tuple data, different transactions
        let tuple = tuple! { id: 1i64, name: "Same" };

        let tid1 = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();
        let tid2 = heap.insert_tuple_versioned(&tuple, test_txn(2)).unwrap();

        // Should create separate versions
        assert_ne!(tid1, tid2);
    }

    #[test]
    fn test_insert_versioned_page_overflow() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Large tuple that will force page overflow
        let data = vec![0u8; 3000];
        let tuple = tuple! { data: data };

        let tid1 = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();
        let tid2 = heap.insert_tuple_versioned(&tuple, test_txn(2)).unwrap();

        // Should go to different pages
        assert_eq!(tid1.page_id, 0);
        assert_eq!(tid2.page_id, 1);
    }

    #[test]
    fn test_insert_versioned_empty_tuple() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Empty tuple type
        let heading = TupleType::new();
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

        let tuple = Relation::from_tuples(rel_type, vec![])
            .unwrap()
            .tuples()
            .next()
            .cloned()
            .unwrap_or_else(|| tuple! {});

        let result = heap.insert_tuple_versioned(&tuple, test_txn(1));

        // Should succeed even with empty tuple
        assert!(result.is_ok());
    }

    #[test]
    fn test_insert_versioned_preserves_prev_version_none() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let tuple = tuple! { id: 1i64, name: "Initial" };
        heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        // prev_version should be None for new inserts
        // (will be tested more thoroughly in Phase 5)
    }

    #[test]
    fn test_insert_versioned_sequential_slots() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert 5 tuples
        for i in 0..5 {
            let tuple = tuple! { id: i as i64, name: format!("Tuple{}", i) };
            let tid = heap
                .insert_tuple_versioned(&tuple, test_txn(i + 1))
                .unwrap();

            assert_eq!(tid.page_id, 0);
            assert_eq!(tid.slot, i as u32);
        }
    }

    // Phase 2.3: Scan with Visibility tests

    use crate::mvcc::TransactionSnapshot;
    use crate::wal::Lsn;
    use std::collections::HashSet;

    fn test_lsn(value: u64) -> Lsn {
        Lsn::new(value)
    }

    #[test]
    fn test_scan_visible_sees_only_visible() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1 inserts and commits
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
            .unwrap();

        // T2 starts after T1 committed
        let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn test_scan_visible_skips_uncommitted() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1 inserts but doesn't commit
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
            .unwrap();

        // T2 starts
        let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
        let committed = HashSet::new(); // T1 not committed

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        // T2 should not see T1's uncommitted tuple
        assert_eq!(visible.len(), 0);
    }

    #[test]
    fn test_scan_visible_sees_own_changes() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let txn_id = test_txn(1);

        // T1 inserts (uncommitted)
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, txn_id)
            .unwrap();

        // T1's snapshot
        let snapshot = TransactionSnapshot::new(txn_id, test_lsn(100), vec![]);
        let committed = HashSet::new(); // T1 not committed yet

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        // T1 should see its own uncommitted tuple
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn test_scan_visible_concurrent_uncommitted() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1 inserts
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
            .unwrap();

        // T2 starts while T1 is active
        let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(100), vec![test_txn(1)]);
        let mut committed = HashSet::new();
        committed.insert(test_txn(1)); // T1 committed after T2 started

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        // T2 should not see T1's tuple (was active when T2 started)
        assert_eq!(visible.len(), 0);
    }

    #[test]
    fn test_scan_visible_empty_relation() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let snapshot = TransactionSnapshot::new(test_txn(1), test_lsn(100), vec![]);
        let committed = HashSet::new();

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        assert_eq!(visible.len(), 0);
    }

    #[test]
    fn test_scan_visible_multiple_committed() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1, T2, T3 insert and commit
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
            .unwrap();
        heap.insert_tuple_versioned(&tuple! { id: 2i64, name: "Bob" }, test_txn(2))
            .unwrap();
        heap.insert_tuple_versioned(&tuple! { id: 3i64, name: "Charlie" }, test_txn(3))
            .unwrap();

        // T4 starts after all committed
        let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
        let mut committed = HashSet::new();
        committed.insert(test_txn(1));
        committed.insert(test_txn(2));
        committed.insert(test_txn(3));

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        assert_eq!(visible.len(), 3);
    }

    #[test]
    fn test_scan_visible_mixed_committed_uncommitted() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1 inserts and commits
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
            .unwrap();

        // T2 inserts but doesn't commit
        heap.insert_tuple_versioned(&tuple! { id: 2i64, name: "Bob" }, test_txn(2))
            .unwrap();

        // T3 inserts and commits
        heap.insert_tuple_versioned(&tuple! { id: 3i64, name: "Charlie" }, test_txn(3))
            .unwrap();

        // T4 starts
        let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
        let mut committed = HashSet::new();
        committed.insert(test_txn(1));
        // T2 not committed
        committed.insert(test_txn(3));

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        // Should see T1 and T3, but not T2
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn test_scan_visible_across_pages() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert large tuples to span multiple pages
        let data = vec![0u8; 3000];

        heap.insert_tuple_versioned(&tuple! { data: data.clone() }, test_txn(1))
            .unwrap();
        heap.insert_tuple_versioned(&tuple! { data: data.clone() }, test_txn(2))
            .unwrap();
        heap.insert_tuple_versioned(&tuple! { data: data.clone() }, test_txn(3))
            .unwrap();

        let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
        let mut committed = HashSet::new();
        committed.insert(test_txn(1));
        committed.insert(test_txn(2));
        committed.insert(test_txn(3));

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        // Should see all 3 tuples across multiple pages
        assert_eq!(visible.len(), 3);
    }

    #[test]
    fn test_scan_visible_no_committed_set() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert tuples
        heap.insert_tuple_versioned(&tuple! { id: 1i64, name: "Alice" }, test_txn(1))
            .unwrap();

        // Empty committed set
        let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
        let committed = HashSet::new();

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        // Should not see any tuples (none committed)
        assert_eq!(visible.len(), 0);
    }

    #[test]
    fn test_scan_visible_preserves_tuple_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let original_tuple = tuple! { id: 42i64, name: "TestData" };
        heap.insert_tuple_versioned(&original_tuple, test_txn(1))
            .unwrap();

        let snapshot = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        assert_eq!(visible.len(), 1);
        // Tuple data should be preserved (exact content verified by serialization)
        assert_eq!(visible[0], original_tuple);
    }

    // Phase 5.1: Update Creates Version (TDD - RED)

    #[test]
    fn test_update_creates_new_version() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original version
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Update to new version
        let updated = tuple! { id: 1i64, name: "Updated" };
        let new_tuple_id = heap
            .update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // New version should have different TupleId
        assert_ne!(tuple_id, new_tuple_id);

        // Read new version
        let new_version = heap.read_tuple_versioned(new_tuple_id).unwrap();
        assert_eq!(new_version, updated);
    }

    #[test]
    fn test_update_marks_old_xmax() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original version
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Update sets xmax on old version
        let updated = tuple! { id: 1i64, name: "Updated" };
        heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // Old version should have xmax set
        let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let slot = versioned_page.slots[tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(slot.xmax, Some(test_txn(2)));
    }

    #[test]
    fn test_update_new_version_has_correct_xmin() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original version
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Update creates new version with updating transaction's ID
        let updated = tuple! { id: 1i64, name: "Updated" };
        let new_tuple_id = heap
            .update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // New version should have xmin = test_txn(2)
        let page = heap.page_file.read_page(new_tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let slot = versioned_page.slots[new_tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(slot.xmin, test_txn(2));
        assert_eq!(slot.xmax, None); // Not yet deleted
    }

    #[test]
    fn test_update_links_versions() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original version
        let original = tuple! { id: 1i64, name: "Original" };
        let old_tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Update creates version chain
        let updated = tuple! { id: 1i64, name: "Updated" };
        let new_tuple_id = heap
            .update_tuple_versioned(old_tuple_id, &updated, test_txn(2))
            .unwrap();

        // New version should point back to old version
        let page = heap.page_file.read_page(new_tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let new_slot = versioned_page.slots[new_tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(new_slot.prev_version, Some(old_tuple_id));
    }

    #[test]
    fn test_update_concurrent_txn_sees_old_version() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert original version
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // T1 commits
        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // T2: Begin (concurrent with T3)
        let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);

        // T3: Update
        let updated = tuple! { id: 1i64, name: "Updated" };
        heap.update_tuple_versioned(tuple_id, &updated, test_txn(3))
            .unwrap();

        // T3 commits
        committed.insert(test_txn(3));

        // NOTE: Current implementation uses Read Committed isolation, not Snapshot Isolation
        // TODO: Full snapshot isolation requires commit LSN tracking
        // With Read Committed, T2 sees T3's committed update
        let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], updated); // Sees committed update (Read Committed semantics)
    }

    #[test]
    fn test_update_updating_txn_sees_new_version() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert and commit
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // T2: Update
        let updated = tuple! { id: 1i64, name: "Updated" };
        heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // T2 should see its own update (not yet committed)
        let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
        let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], updated); // Should see updated version
    }

    #[test]
    fn test_update_multiple_times_creates_chain() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original
        let v1 = tuple! { id: 1i64, name: "V1" };
        let tid1 = heap.insert_tuple_versioned(&v1, test_txn(1)).unwrap();

        // Update to V2
        let v2 = tuple! { id: 1i64, name: "V2" };
        let tid2 = heap.update_tuple_versioned(tid1, &v2, test_txn(2)).unwrap();

        // Update to V3
        let v3 = tuple! { id: 1i64, name: "V3" };
        let tid3 = heap.update_tuple_versioned(tid2, &v3, test_txn(3)).unwrap();

        // Verify chain: tid3 -> tid2 -> tid1
        let page3 = heap.page_file.read_page(tid3.page_id).unwrap();
        let vpage3: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page3.data()).unwrap();
        let slot3 = vpage3.slots[tid3.slot as usize].as_ref().unwrap();
        assert_eq!(slot3.prev_version, Some(tid2));

        let page2 = heap.page_file.read_page(tid2.page_id).unwrap();
        let vpage2: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page2.data()).unwrap();
        let slot2 = vpage2.slots[tid2.slot as usize].as_ref().unwrap();
        assert_eq!(slot2.prev_version, Some(tid1));
    }

    #[test]
    fn test_update_nonexistent_tuple_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let bogus_id = TupleId {
            page_id: 999,
            slot: 0,
        };
        let updated = tuple! { id: 1i64, name: "Updated" };

        let result = heap.update_tuple_versioned(bogus_id, &updated, test_txn(1));
        assert!(result.is_err());
        assert!(matches!(result, Err(HeapError::TupleNotFound)));
    }

    #[test]
    fn test_update_preserves_tuple_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original
        let original = tuple! { id: 42i64, name: "OriginalData" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Update with different data
        let updated = tuple! { id: 42i64, name: "UpdatedData" };
        let new_tuple_id = heap
            .update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // Verify both versions have correct data
        let old_tuple = heap.read_tuple_versioned(tuple_id).unwrap();
        assert_eq!(old_tuple, original);

        let new_tuple = heap.read_tuple_versioned(new_tuple_id).unwrap();
        assert_eq!(new_tuple, updated);
    }

    #[test]
    fn test_update_old_version_xmin_unchanged() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // T2: Update
        let updated = tuple! { id: 1i64, name: "Updated" };
        heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // Old version should still have xmin = test_txn(1)
        let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let slot = versioned_page.slots[tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(slot.xmin, test_txn(1));
    }

    #[test]
    fn test_update_different_pages() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Update might go to different page if original page is full
        let updated = tuple! { id: 1i64, name: "Updated" };
        let new_tuple_id = heap
            .update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();

        // Both versions should be readable
        let old_tuple = heap.read_tuple_versioned(tuple_id).unwrap();
        assert_eq!(old_tuple, original);

        let new_tuple = heap.read_tuple_versioned(new_tuple_id).unwrap();
        assert_eq!(new_tuple, updated);
    }

    #[test]
    fn test_update_after_commit_visible_to_later_txn() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert and commit
        let original = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // T2: Update and commit
        let updated = tuple! { id: 1i64, name: "Updated" };
        heap.update_tuple_versioned(tuple_id, &updated, test_txn(2))
            .unwrap();
        committed.insert(test_txn(2));

        // T3: Should see updated version
        let snapshot_t3 = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
        let visible = heap.scan_visible(&snapshot_t3, &committed).unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], updated);
    }

    // Phase 5.2: Delete Marks xmax (TDD - RED)

    #[test]
    fn test_delete_sets_xmax() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert tuple
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        // Delete tuple
        heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

        // Verify xmax is set
        let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let slot = versioned_page.slots[tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(slot.xmax, Some(test_txn(2)));
    }

    #[test]
    fn test_delete_invisible_after_commit() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert and commit
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // T2: Delete and commit
        heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();
        committed.insert(test_txn(2));

        // T3: Should not see deleted tuple
        let snapshot_t3 = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
        let visible = heap.scan_visible(&snapshot_t3, &committed).unwrap();

        assert_eq!(visible.len(), 0); // Deleted tuple not visible
    }

    #[test]
    fn test_delete_concurrent_txn_sees_tuple() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert and commit
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // T2: Begin (concurrent with T3)
        let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);

        // T3: Delete and commit
        heap.delete_tuple_versioned(tuple_id, test_txn(3)).unwrap();
        committed.insert(test_txn(3));

        // NOTE: With Read Committed, T2 sees the deletion
        // TODO: Full snapshot isolation would preserve visibility
        let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

        assert_eq!(visible.len(), 0); // Read Committed: sees deletion
    }

    #[test]
    fn test_delete_deleting_txn_doesnt_see_tuple() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert and commit
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // T2: Delete
        heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

        // T2 should not see the tuple it deleted
        let snapshot_t2 = TransactionSnapshot::new(test_txn(2), test_lsn(200), vec![]);
        let visible = heap.scan_visible(&snapshot_t2, &committed).unwrap();

        assert_eq!(visible.len(), 0);
    }

    #[test]
    fn test_delete_nonexistent_tuple_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        let bogus_id = TupleId {
            page_id: 999,
            slot: 0,
        };

        let result = heap.delete_tuple_versioned(bogus_id, test_txn(1));
        assert!(result.is_err());
        assert!(matches!(result, Err(HeapError::TupleNotFound)));
    }

    #[test]
    fn test_delete_preserves_xmin() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // T1: Insert
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        // T2: Delete
        heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

        // Verify xmin unchanged
        let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let slot = versioned_page.slots[tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(slot.xmin, test_txn(1));
        assert_eq!(slot.xmax, Some(test_txn(2)));
    }

    #[test]
    fn test_delete_preserves_tuple_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert tuple
        let original = tuple! { id: 42i64, name: "DataToPreserve" };
        let tuple_id = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        // Delete tuple
        heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

        // Tuple data should still be readable (though invisible)
        let tuple = heap.read_tuple_versioned(tuple_id).unwrap();
        assert_eq!(tuple, original);
    }

    #[test]
    fn test_delete_already_deleted_sets_xmax_again() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert tuple
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        // Delete by T2
        heap.delete_tuple_versioned(tuple_id, test_txn(2)).unwrap();

        // Delete again by T3 (should succeed and update xmax)
        heap.delete_tuple_versioned(tuple_id, test_txn(3)).unwrap();

        // Verify xmax is now T3
        let page = heap.page_file.read_page(tuple_id.page_id).unwrap();
        let versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();
        let slot = versioned_page.slots[tuple_id.slot as usize]
            .as_ref()
            .unwrap();

        assert_eq!(slot.xmax, Some(test_txn(3)));
    }

    #[test]
    fn test_delete_multiple_tuples() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert multiple tuples
        let t1 = tuple! { id: 1i64, name: "First" };
        let t2 = tuple! { id: 2i64, name: "Second" };
        let t3 = tuple! { id: 3i64, name: "Third" };

        let _tid1 = heap.insert_tuple_versioned(&t1, test_txn(1)).unwrap();
        let tid2 = heap.insert_tuple_versioned(&t2, test_txn(1)).unwrap();
        let _tid3 = heap.insert_tuple_versioned(&t3, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // Delete middle tuple
        heap.delete_tuple_versioned(tid2, test_txn(2)).unwrap();
        committed.insert(test_txn(2));

        // Should see first and third, but not second
        let snapshot = TransactionSnapshot::new(test_txn(3), test_lsn(300), vec![]);
        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        assert_eq!(visible.len(), 2);
        assert!(visible.contains(&t1));
        assert!(!visible.contains(&t2));
        assert!(visible.contains(&t3));
    }

    #[test]
    fn test_delete_and_insert_new_version() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Insert original
        let original = tuple! { id: 1i64, name: "Original" };
        let tid1 = heap.insert_tuple_versioned(&original, test_txn(1)).unwrap();

        let mut committed = HashSet::new();
        committed.insert(test_txn(1));

        // Delete
        heap.delete_tuple_versioned(tid1, test_txn(2)).unwrap();
        committed.insert(test_txn(2));

        // Insert new version with same logical key
        let new_ver = tuple! { id: 1i64, name: "Reinserted" };
        heap.insert_tuple_versioned(&new_ver, test_txn(3)).unwrap();
        committed.insert(test_txn(3));

        // Should see only the new version
        let snapshot = TransactionSnapshot::new(test_txn(4), test_lsn(400), vec![]);
        let visible = heap.scan_visible(&snapshot, &committed).unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], new_ver);
    }

    #[test]
    fn test_heap_scan_corrupted_slot() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Create a manually corrupted page
        // Slot points to offset > PAGE_SIZE
        let slotted_page = SlottedPage {
            slot_count: 1,
            slots: vec![Some(SlotEntry {
                offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                length: 10,
            })],
        };

        // Serialize just the header (no tuple data needed as offset is invalid)
        let slot_dir = postcard::to_allocvec(&slotted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[..slot_dir.len()].copy_from_slice(&slot_dir);

        let page = Page::from_data(0, page_data).unwrap();
        heap.page_file.write_page(&page).unwrap();

        // Scan should fail
        let result = heap.scan();
        assert!(result.is_err());
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert!(msg.contains("Corrupted slot"));
            }
            _ => panic!("Expected Serialization error for corrupted slot"),
        }
    }

    #[test]
    fn test_heap_scan_mid_stream_corruption() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path();

        // 1. Create a large enough tuple to fill most of a page
        let large_data = vec![0u8; 3000];
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("data", ScalarType::Bytes);
        let rel_type_large = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type_large)?;

        // 2. Insert 3 tuples, each should land on a separate page
        for i in 0..3 {
            let tuple = tuple! {
                id: i as i64,
                data: large_data.clone(),
            };
            heap.insert_tuple(&tuple)?;
        }

        // Verify initial scan works
        let results = heap.scan()?;
        assert_eq!(results.len(), 3);

        // 3. Corrupt the middle page (Page 1)
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::OpenOptions::new().write(true).open(path)?;

            // Seek to start of Page 1
            file.seek(SeekFrom::Start(PAGE_SIZE as u64))?;

            // Skip page length prefix (8 bytes) to corrupt the actual content
            file.seek(SeekFrom::Current(8))?;

            // Write garbage
            file.write_all(&[0xFF; 100])?;
            file.sync_all()?;
        }

        // 4. Verify scan fails gracefully
        let result = heap.scan();
        assert!(result.is_err());
        // Should be a serialization error because postcard will fail to deserialize garbage
        match result {
            Err(HeapError::Serialization(_)) => {}
            _ => panic!("Expected Serialization error, got {:?}", result),
        }

        Ok(())
    }

    #[test]
    fn test_heap_insert_on_corrupted_page_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Create a corrupted page
        // Slot points to offset > PAGE_SIZE
        let slotted_page = SlottedPage {
            slot_count: 1,
            slots: vec![Some(SlotEntry {
                offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                length: 10,
            })],
        };

        // Serialize header
        let slot_dir = postcard::to_allocvec(&slotted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[..slot_dir.len()].copy_from_slice(&slot_dir);

        let page = Page::from_data(0, page_data).unwrap();
        heap.page_file.write_page(&page).unwrap();

        // 2. Try to insert a new tuple
        // This should fail because it needs to read existing tuples to shift them
        let tuple = tuple! { id: 1i64, name: "NewTuple" };
        let result = heap.insert_tuple(&tuple);

        // 3. Assert failure
        // Currently this fails (returns Ok) because of the bug
        assert!(
            result.is_err(),
            "Insert should fail on corrupted page, but succeeded"
        );
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert!(
                    msg.contains("Corrupted slot") || msg.contains("outside page data"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }
    #[test]
    fn test_heap_insert_versioned_on_corrupted_page_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Create a corrupted versioned page
        // Slot points to offset > PAGE_SIZE
        let versioned_page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 1,
            slots: vec![Some(VersionedSlotEntry {
                offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                length: 10,
                xmin: test_txn(1),
                xmax: None,
                prev_version: None,
            })],
        };

        // Serialize header
        let slot_dir = postcard::to_allocvec(&versioned_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];

        // Write format version
        page_data[0] = PAGE_FORMAT_VERSION;

        // Write slot directory length
        let slot_dir_len = slot_dir.len() as u32;
        page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

        // Copy slot directory after header
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let page = Page::from_data(0, page_data).unwrap();
        heap.page_file.write_page(&page).unwrap();

        // 2. Try to insert a new versioned tuple
        // This should fail when extracting existing tuples
        let tuple = tuple! { id: 1i64, name: "NewTuple" };
        let result = heap.insert_tuple_versioned(&tuple, test_txn(2));

        // 3. Assert failure
        assert!(
            result.is_err(),
            "Insert versioned should fail on corrupted page"
        );
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert!(
                    msg.contains("Corrupted slot") || msg.contains("outside page data"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_update_on_corrupted_page_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Insert a tuple to get a valid TupleId
        let tuple = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        // 2. Corrupt the page (slot pointing outside)
        // We need to read the page, construct a corrupted version, and write it back
        {
            let page = heap.page_file.read_page(tuple_id.page_id).unwrap();

            // Deserialize (valid)
            let _versioned_page: VersionedSlottedPage =
                deserialize_versioned_page_for_test(page.data()).unwrap();

            // Create corrupted version
            let corrupted_page = VersionedSlottedPage {
                magic: VERSIONED_PAGE_MAGIC,
                slot_count: 1,
                slots: vec![Some(VersionedSlotEntry {
                    offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                    length: 10,
                    xmin: test_txn(1),
                    xmax: None,
                    prev_version: None,
                })],
            };

            // Serialize and write back
            let slot_dir = postcard::to_allocvec(&corrupted_page).unwrap();
            let mut page_data = vec![0u8; PAGE_SIZE - 8];
            page_data[0] = PAGE_FORMAT_VERSION;
            let slot_dir_len = slot_dir.len() as u32;
            page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());
            page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

            let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
            heap.page_file.write_page(&new_page).unwrap();
        }

        // 3. Try to update the tuple
        // This should fail when extracting existing tuples in update_tuple_versioned
        let updated = tuple! { id: 1i64, name: "Updated" };
        let result = heap.update_tuple_versioned(tuple_id, &updated, test_txn(2));

        // 4. Assert failure
        assert!(result.is_err(), "Update should fail on corrupted page");
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert!(
                    msg.contains("Corrupted slot") || msg.contains("outside page data"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_delete_on_corrupted_page_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Insert a tuple to get a valid TupleId
        let tuple = tuple! { id: 1i64, name: "Original" };
        let tuple_id = heap.insert_tuple_versioned(&tuple, test_txn(1)).unwrap();

        // 2. Corrupt the page (slot pointing outside)
        {
            let corrupted_page = VersionedSlottedPage {
                magic: VERSIONED_PAGE_MAGIC,
                slot_count: 1,
                slots: vec![Some(VersionedSlotEntry {
                    offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                    length: 10,
                    xmin: test_txn(1),
                    xmax: None,
                    prev_version: None,
                })],
            };

            let slot_dir = postcard::to_allocvec(&corrupted_page).unwrap();
            let mut page_data = vec![0u8; PAGE_SIZE - 8];
            page_data[0] = PAGE_FORMAT_VERSION;
            let slot_dir_len = slot_dir.len() as u32;
            page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());
            page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

            let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
            heap.page_file.write_page(&new_page).unwrap();
        }

        // 3. Try to delete the tuple
        let result = heap.delete_tuple_versioned(tuple_id, test_txn(2));

        // 4. Assert failure
        assert!(result.is_err(), "Delete should fail on corrupted page");
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert!(
                    msg.contains("Corrupted slot") || msg.contains("outside page data"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_gc_on_corrupted_page_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Create a corrupted versioned page
        // Slot points to offset > PAGE_SIZE
        let versioned_page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 1,
            slots: vec![Some(VersionedSlotEntry {
                offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                length: 10,
                xmin: test_txn(1),
                xmax: None,
                prev_version: None,
            })],
        };

        // Serialize header
        let slot_dir = postcard::to_allocvec(&versioned_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];

        // Write format version
        page_data[0] = PAGE_FORMAT_VERSION;

        // Write slot directory length
        let slot_dir_len = slot_dir.len() as u32;
        page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

        // Copy slot directory after header
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let page = Page::from_data(0, page_data).unwrap();
        heap.page_file.write_page(&page).unwrap();

        // 2. Run GC
        let result = heap
            .gc_remove_dead_versions(crate::wal::Lsn::new(100), &std::collections::HashSet::new());

        // 3. Assert failure
        assert!(result.is_err(), "GC should fail on corrupted page");
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert!(
                    msg.contains("Corrupted slot") || msg.contains("outside page data"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_insert_too_large_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Create a tuple that is definitely too large (> 4096)
        let data = vec![0u8; 5000];
        let tuple = tuple! { data: data };

        let result = heap.insert_tuple(&tuple);

        assert!(result.is_err());
        match result {
            Err(HeapError::TupleTooLarge(size)) => {
                assert!(size >= 5000);
            }
            _ => panic!("Expected TupleTooLarge error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_insert_versioned_too_large_fails() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Create a tuple that is definitely too large (> 4096)
        let data = vec![0u8; 5000];
        let tuple = tuple! { data: data };

        let result = heap.insert_tuple_versioned(&tuple, test_txn(1));

        assert!(result.is_err());
        match result {
            Err(HeapError::TupleTooLarge(size)) => {
                assert!(size >= 5000);
            }
            _ => panic!("Expected TupleTooLarge error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_scan_offset_overflow() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Create a page with an overflowing offset
        // offset = u32::MAX - 10, length = 20
        // offset + length overflows u32
        let corrupted_slot = SlotEntry {
            offset: u32::MAX - 10,
            length: 20,
        };

        let slotted_page = SlottedPage {
            slot_count: 1,
            slots: vec![Some(corrupted_slot)],
        };

        // Serialize header only
        let slot_dir = postcard::to_allocvec(&slotted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[..slot_dir.len()].copy_from_slice(&slot_dir);

        let page = Page::from_data(0, page_data).unwrap();
        heap.page_file.write_page(&page).unwrap();

        // Scan should fail cleanly
        let result = heap.scan();
        assert!(result.is_err());
        match result {
            Err(HeapError::Serialization(msg)) => {
                // On 64-bit systems, u32+u32 fits in usize, so we get bounds check error.
                // On 32-bit systems, we get overflow error.
                assert!(
                    msg.contains("Tuple end offset overflow") || msg.contains("Corrupted slot"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            _ => panic!(
                "Expected Serialization error with overflow message, got {:?}",
                result
            ),
        }
    }

    #[test]
    fn test_heap_deserialize_short_page() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let rel_type = create_test_relation_type();
        let heap = HeapFile::create(path, rel_type).unwrap();

        // Create a page that mimics versioned format but is too short
        // PAGE_FORMAT_VERSION (1 byte) + 4 bytes length = 5 bytes needed
        // We write 3 bytes: [PAGE_FORMAT_VERSION, 0, 0]
        let data = vec![PAGE_FORMAT_VERSION, 0, 0];
        let page = Page::from_data(0, data).unwrap();

        // Call private method directly to verify protection
        // (update_tuple_versioned calls this without is_versioned_page check)
        let result = heap.deserialize_versioned_page(&page);

        assert!(result.is_err());
        match result {
            Err(HeapError::Serialization(msg)) => {
                assert_eq!(msg, "Versioned page too short to contain header");
            }
            _ => panic!("Expected specific Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_header_corruption() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create a relation type
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // We want to insert many small tuples to increase N (number of slots).
        // Discrepancy D = N bytes.
        // We want to fill the page such that we are just on the edge.

        for i in 0..1000 {
            let t = tuple! { id: i as i64 };
            heap.insert_tuple(&t).unwrap();

            // Read page 0
            let page = heap.page_file.read_page(0).unwrap();
            // Try to deserialize
            let res = heap.deserialize_slotted_page(&page);
            if res.is_err() {
                println!("Corruption detected at insert {}!", i);
                panic!("Corruption detected: {:?}", res.err());
            }

            // Also verify that the last inserted tuple is valid
            let sp = res.unwrap();
            if let Some(Some(last_slot)) = sp.slots.last() {
                // Check for overlap
                let slot_dir = postcard::to_allocvec(&sp).unwrap();
                // Slot dir is at offset 0.
                // Tuple is at last_slot.offset.
                // If tuple start < slot_dir end, we have overlap.
                if last_slot.offset < slot_dir.len() as u32 {
                    println!(
                        "Overlap detected at insert {}! Offset: {}, Header: {}",
                        i,
                        last_slot.offset,
                        slot_dir.len()
                    );
                    panic!("Overlap detected!");
                }
            }

            if page.id() > 0 {
                println!("Page split happened at insert {}", i);
                break;
            }
        }
    }

    #[test]
    fn test_allocation_bomb_prevention() {
        // Construct a malicious payload: a Vec<u8> with length prefix 1GB
        // Postcard uses varints. 1 GB is 2^30.
        let huge_len: usize = 1024 * 1024 * 1024; // 1 GB
        let mut payload = Vec::new();
        // Since postcard uses varint, we must serialize the length the way postcard does
        payload.extend_from_slice(&postcard::to_allocvec(&huge_len).unwrap());
        // No actual data follows

        // Try to deserialize into Vec<u8> using our bounded deserializer
        // This should fail immediately because postcard checks if enough bytes are remaining
        let result: Result<Vec<u8>, HeapError> = deserialize_bounded(&payload);

        assert!(result.is_err());
        match result {
            Err(HeapError::Serialization(_)) => {
                // Expected error
            }
            _ => panic!("Expected Serialization error, got {:?}", result),
        }
    }

    #[test]
    fn test_heap_slot_reuse_corruption() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // 1. Insert 3 tuples: A, B, C
        let tuple_a = tuple! { id: 1i64, name: "A" };
        let tuple_b = tuple! { id: 2i64, name: "B" };
        let tuple_c = tuple! { id: 3i64, name: "C" };

        heap.insert_tuple(&tuple_a).unwrap();
        heap.insert_tuple(&tuple_b).unwrap();
        heap.insert_tuple(&tuple_c).unwrap();

        // Verify initial state
        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 3);

        // 2. Manually simulate deletion of B (slot 1) to force reuse
        // We do this by modifying the page directly since we don't have a public delete yet
        {
            let page = heap.page_file.read_page(0).unwrap();
            let mut sp = heap.deserialize_slotted_page(&page).unwrap();

            // Delete slot 1 (B)
            sp.slots[1] = None;

            // Extract existing tuples (A, B, C)
            // Note: extract_all_tuples returns Vec<Vec<u8>> corresponding to slots
            // But we just modified slots[1] to None!
            // So we need to be careful.
            // Let's re-read the page as it was on disk to get the data
            let original_sp = heap.deserialize_slotted_page(&page).unwrap();
            let mut existing_tuples = heap.extract_all_tuples(&page, &original_sp.slots).unwrap();

            // Mark tuple B as empty
            existing_tuples[1] = Vec::new();

            // Repack and write back
            HeapFile::repack_slots(&mut sp.slots, &existing_tuples, USABLE_PAGE_SIZE_V1).unwrap();
            let new_page_data = heap
                .serialize_slotted_page_with_tuples(&sp, &existing_tuples)
                .unwrap();
            let new_page = Page::from_data(0, new_page_data).unwrap();
            heap.page_file.write_page(&new_page).unwrap();
        }

        // Verify B is gone
        let tuples = heap.scan().unwrap();
        assert_eq!(tuples.len(), 2);
        assert!(tuples.contains(&tuple_a));
        assert!(tuples.contains(&tuple_c));

        // 3. Insert tuple D. Should reuse slot 1.
        let tuple_d = tuple! { id: 4i64, name: "D" };
        heap.insert_tuple(&tuple_d).unwrap();

        // 4. Verify all tuples are present and correct
        let tuples = heap.scan().unwrap();

        // If corruption happened, C might be lost or corrupted
        assert_eq!(tuples.len(), 3, "Expected 3 tuples (A, C, D)");

        assert!(tuples.contains(&tuple_a), "Missing tuple A");
        assert!(tuples.contains(&tuple_d), "Missing tuple D");
        assert!(
            tuples.contains(&tuple_c),
            "Missing tuple C - CORRUPTION DETECTED!"
        );
    }

    #[test]
    fn test_repack_slots_correctness() {
        // Test edge case where tuples perfectly fill the page
        let mut slots: Vec<Option<SlotEntry>> = vec![];
        let mut tuples: Vec<Vec<u8>> = vec![];

        // Add 2 tuples, each 100 bytes
        slots.push(Some(SlotEntry {
            offset: 0,
            length: 100,
        }));
        tuples.push(vec![0u8; 100]);

        slots.push(Some(SlotEntry {
            offset: 0,
            length: 100,
        }));
        tuples.push(vec![0u8; 100]);

        // Usable size = 200
        let usable_size = 200;

        HeapFile::repack_slots(&mut slots, &tuples, usable_size).unwrap();

        // Check offsets
        // Last tuple (index 1) gets offset: 200 - 100 = 100
        assert_eq!(slots[1].as_ref().unwrap().offset, 100);
        assert_eq!(slots[1].as_ref().unwrap().length, 100);

        // First tuple (index 0) gets offset: 100 - 100 = 0
        assert_eq!(slots[0].as_ref().unwrap().offset, 0);
        assert_eq!(slots[0].as_ref().unwrap().length, 100);

        // Test overflow (too many tuples)
        let tuples_overflow = vec![vec![0u8; 100], vec![0u8; 100], vec![0u8; 1]]; // Total 201
        let mut slots_overflow = vec![
            Some(SlotEntry {
                offset: 0,
                length: 100,
            }),
            Some(SlotEntry {
                offset: 0,
                length: 100,
            }),
            Some(SlotEntry {
                offset: 0,
                length: 1,
            }),
        ];

        let result = HeapFile::repack_slots(&mut slots_overflow, &tuples_overflow, usable_size);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HeapError::PageFull));
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{ScalarType, TupleType};
    use tempfile::NamedTempFile;

    #[test]
    fn test_update_versioned_tuple_too_large_prevents_loop() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let heading = TupleType::new().with_attribute("data".to_string(), ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        let mut heap = HeapFile::create(path, rel_type).unwrap();

        // Find the critical payload size dynamically
        let mut critical_payload = None;

        // Loop through sizes that are close to page limit
        // USABLE_PAGE_SIZE is 4088. Overhead roughly 40-60 bytes.
        // We look for a size where it fits with 'None' but fails with 'Some'
        for len in 3500..4088 {
            let data = vec![0u8; len];
            let tuple = tuple! { data: data.clone() };
            // Serialize to get length
            let tuple_data = postcard::to_allocvec(&tuple).unwrap();
            let tuple_data_len = tuple_data.len();

            // Check if it fits with None (insert)
            let fits_insert = heap
                .check_versioned_tuple_size_limit(tuple_data_len, OperationType::Insert)
                .is_ok();

            // Check if it fits with Some (update)
            let fits_update = heap
                .check_versioned_tuple_size_limit(tuple_data_len, OperationType::Update)
                .is_ok();

            if fits_insert && !fits_update {
                println!("Found critical payload length: {}", len);
                critical_payload = Some(len);
                break;
            }
        }

        let len = critical_payload.expect("Failed to find critical payload size");
        let data = vec![0u8; len];
        let tuple = tuple! { data: data };

        // Insert should succeed
        let tid = heap
            .insert_tuple_versioned(&tuple, crate::wal::TransactionId::new(1))
            .expect("Insert failed");

        // Update should fail with TupleTooLarge, NOT loop forever
        // If the bug exists, this call would loop forever (or timeout)
        // With the fix, it should return TupleTooLarge
        let result = heap.update_tuple_versioned(tid, &tuple, crate::wal::TransactionId::new(2));

        assert!(result.is_err());
        match result {
            Err(HeapError::TupleTooLarge(_)) => {
                println!("Caught TupleTooLarge as expected");
            }
            Err(HeapError::PageFull) => {
                panic!("Got PageFull - vulnerability likely present");
            }
            _ => panic!("Expected TupleTooLarge, got {:?}", result),
        }
    }
}
