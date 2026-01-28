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

/// Page layout: [slot_count (4 bytes)] [slot_entries...] [free_space] [...tuple_data]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlottedPage {
    slot_count: u32,
    slots: Vec<Option<SlotEntry>>,
}

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
        let tuple_data =
            bincode::serialize(tuple).map_err(|e| HeapError::Serialization(e.to_string()))?;

        // Find a page with enough space, or create a new one
        let mut page_id = 0;
        loop {
            match self.try_insert_into_page(page_id, &tuple_data) {
                Ok(_slot) => {
                    return Ok(()); // Discard slot, return success
                }
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
        let mut existing_tuples: Vec<Vec<u8>> = Vec::new();
        let mut slotted_page = if page.is_empty() {
            SlottedPage {
                slot_count: 0,
                slots: Vec::new(),
            }
        } else {
            let sp: SlottedPage = bincode::deserialize(page.data())
                .map_err(|e| HeapError::Serialization(e.to_string()))?;

            // Extract existing tuple data
            for slot_entry in sp.slots.iter().flatten() {
                let start = slot_entry.offset as usize;
                let end = start + slot_entry.length as usize;
                if end <= page.data().len() {
                    existing_tuples.push(page.data()[start..end].to_vec());
                } else {
                    existing_tuples.push(Vec::new());
                }
            }

            sp
        };

        // Calculate required space (account for 8-byte length prefix in page format)
        const USABLE_PAGE_SIZE: usize = PAGE_SIZE - 8;
        let slot_entry_size = std::mem::size_of::<SlotEntry>();
        let header_size =
            std::mem::size_of::<u32>() + (slotted_page.slots.len() + 1) * slot_entry_size;
        let total_tuple_data_size: usize =
            existing_tuples.iter().map(|t| t.len()).sum::<usize>() + tuple_data.len();
        let required_space = header_size + total_tuple_data_size;

        if required_space > USABLE_PAGE_SIZE {
            return Err(HeapError::PageFull);
        }

        // Find free slot or add new one
        let slot_number = if let Some(pos) = slotted_page.slots.iter().position(|s| s.is_none()) {
            pos as u32
        } else {
            let new_slot = slotted_page.slots.len() as u32;
            slotted_page.slots.push(None);
            slotted_page.slot_count += 1;
            new_slot
        };

        // Add new tuple to the list
        existing_tuples.insert(slot_number as usize, tuple_data.to_vec());

        // Calculate offsets for all tuples (grow from end backward)
        let mut current_offset = USABLE_PAGE_SIZE;
        for (idx, tuple) in existing_tuples.iter().enumerate().rev() {
            if !tuple.is_empty() {
                current_offset -= tuple.len();
                slotted_page.slots[idx] = Some(SlotEntry {
                    offset: current_offset as u32,
                    length: tuple.len() as u32,
                });
            }
        }

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
        let slot_dir = bincode::serialize(slotted_page)
            .map_err(|e| HeapError::Serialization(e.to_string()))?;

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
    pub(crate) fn read_tuple(&mut self, tuple_id: TupleId) -> Result<Tuple, HeapError> {
        let page = self.page_file.read_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound);
        }

        let slotted_page: SlottedPage = bincode::deserialize(page.data())
            .map_err(|e| HeapError::Serialization(e.to_string()))?;

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
        let tuple: Tuple = bincode::deserialize(tuple_data)
            .map_err(|e| HeapError::Serialization(e.to_string()))?;

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

            // Try to deserialize as slotted page
            let slotted_page: SlottedPage = match bincode::deserialize(page.data()) {
                Ok(sp) => sp,
                Err(_) => {
                    page_id += 1;
                    continue;
                }
            };

            // Read all tuples from this page
            for (slot, slot_entry) in slotted_page.slots.iter().enumerate() {
                if slot_entry.is_some() {
                    let tuple_id = TupleId {
                        page_id,
                        slot: slot as u32,
                    };
                    // TupleId used internally, not exposed
                    if let Ok(tuple) = self.read_tuple(tuple_id) {
                        results.push(tuple); // Only push tuple
                    }
                }
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
            let tuple_data = bincode::serialize(&tuple).unwrap();

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
}
