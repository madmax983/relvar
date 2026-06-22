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
    offset: u32,
    length: u32,
}

/// Versioned slot directory entry for MVCC.
///
/// Extends SlotEntry with transaction version metadata to support
/// Multi-Version Concurrency Control (MVCC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct VersionedSlotEntry {
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
pub(crate) struct SlottedPage {
    slot_count: u32,
    slots: Vec<Option<SlotEntry>>,
}

/// Versioned page layout for MVCC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VersionedSlottedPage {
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

mod insert;
mod read;
mod update;
mod gc;
mod serialization;

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
        let page_file = PageFile::open(path)?;
        Ok(Self {
            page_file,
            relation_type,
        })
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
        self.page_file.sync()?;
        Ok(())
    }

}

#[cfg(test)]
mod tests;
