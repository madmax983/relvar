use super::super::page::{PAGE_SIZE, PageError, PageFile, PageId};
use relvar_core::types::RelationType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Tuple ID: (page_id, slot_number)
/// Internal to storage layer only (TTM Proscription 6)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct TupleId {
    pub(crate) page_id: PageId,
    pub(crate) slot: u32,
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
pub struct HeapFile {
    /// The underlying page file for storage.
    pub(crate) page_file: PageFile,
    /// The type of tuples stored in this heap file.
    pub(crate) relation_type: RelationType,
}

/// Slot directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SlotEntry {
    pub(crate) offset: u32,
    pub(crate) length: u32,
}

/// Versioned slot directory entry for MVCC.
///
/// Extends SlotEntry with transaction version metadata to support
/// Multi-Version Concurrency Control (MVCC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct VersionedSlotEntry {
    /// Offset of tuple data in page
    pub(crate) offset: u32,
    /// Length of tuple data
    pub(crate) length: u32,
    /// Transaction that created this version
    pub(crate) xmin: crate::wal::TransactionId,
    /// Transaction that deleted/updated this version (None = still visible)
    pub(crate) xmax: Option<crate::wal::TransactionId>,
    /// Previous version in the version chain (for undo)
    pub(crate) prev_version: Option<TupleId>,
}

impl SlotEntry {
    pub(crate) fn offset(&self) -> u32 {
        self.offset
    }

    pub(crate) fn length(&self) -> u32 {
        self.length
    }

    pub(crate) fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    pub(crate) fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

impl VersionedSlotEntry {
    pub(crate) fn offset(&self) -> u32 {
        self.offset
    }

    pub(crate) fn length(&self) -> u32 {
        self.length
    }

    pub(crate) fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    pub(crate) fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

/// Page layout: `[slot_count (4 bytes)] [slot_entries...] [free_space] [...tuple_data]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SlottedPage {
    pub(crate) slot_count: u32,
    pub(crate) slots: Vec<Option<SlotEntry>>,
}

/// Versioned page layout for MVCC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VersionedSlottedPage {
    pub(crate) magic: u32, // Magic number to distinguish from SlottedPage: 0x4D564343 ("MVCC")
    pub(crate) slot_count: u32,
    pub(crate) slots: Vec<Option<VersionedSlotEntry>>,
}

pub(crate) const VERSIONED_PAGE_MAGIC: u32 = 0x4D564343; // "MVCC" in ASCII

// Page format version to handle serialization changes
pub(crate) const PAGE_FORMAT_VERSION: u8 = 2; // Version 2: length-prefixed slot directory

pub(crate) const USABLE_PAGE_SIZE_V1: usize = PAGE_SIZE - 8;
pub(crate) const USABLE_PAGE_SIZE_V2: usize = PAGE_SIZE - 8;
pub(crate) const V2_HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length
