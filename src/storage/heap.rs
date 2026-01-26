use crate::storage::page::{PAGE_SIZE, Page, PageError, PageFile, PageId};
use crate::types::RelationType;
use crate::values::{Relation, Tuple};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Tuple ID: (page_id, slot_number)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TupleId {
    pub page_id: PageId,
    pub slot: u32,
}

#[derive(Debug, Error)]
pub enum HeapError {
    #[error("Page error: {0}")]
    Page(#[from] PageError),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Tuple not found: {0:?}")]
    TupleNotFound(TupleId),
    #[error("Page full")]
    PageFull,
}

/// Heap file stores tuples in an unordered collection of pages.
/// Each page contains a slot directory and tuple data.
pub struct HeapFile {
    page_file: PageFile,
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
    /// Create a new heap file
    pub fn create<P: AsRef<Path>>(path: P, relation_type: RelationType) -> Result<Self, HeapError> {
        let page_file = PageFile::create(path)?;
        Ok(Self {
            page_file,
            relation_type,
        })
    }

    /// Open an existing heap file
    pub fn open<P: AsRef<Path>>(path: P, relation_type: RelationType) -> Result<Self, HeapError> {
        let page_file = PageFile::open(path)?;
        Ok(Self {
            page_file,
            relation_type,
        })
    }

    /// Insert a tuple and return its TupleId
    pub fn insert_tuple(&mut self, tuple: &Tuple) -> Result<TupleId, HeapError> {
        // Serialize the tuple
        let tuple_data =
            bincode::serialize(tuple).map_err(|e| HeapError::Serialization(e.to_string()))?;

        // Find a page with enough space, or create a new one
        let mut page_id = 0;
        loop {
            match self.try_insert_into_page(page_id, &tuple_data) {
                Ok(slot) => {
                    return Ok(TupleId { page_id, slot });
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
        let page = self
            .page_file
            .read_page(page_id)
            .unwrap_or_else(|_| Page::new(page_id));

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
        let total_tuple_data_size: usize = existing_tuples.iter().map(|t| t.len()).sum::<usize>() + tuple_data.len();
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

    /// Read a tuple by its TupleId
    pub fn read_tuple(&mut self, tuple_id: TupleId) -> Result<Tuple, HeapError> {
        let page = self.page_file.read_page(tuple_id.page_id)?;

        if page.is_empty() {
            return Err(HeapError::TupleNotFound(tuple_id));
        }

        let slotted_page: SlottedPage = bincode::deserialize(page.data())
            .map_err(|e| HeapError::Serialization(e.to_string()))?;

        let slot_entry = slotted_page
            .slots
            .get(tuple_id.slot as usize)
            .and_then(|s| s.as_ref())
            .ok_or(HeapError::TupleNotFound(tuple_id))?;

        // Extract tuple data from page
        let start = slot_entry.offset as usize;
        let end = start + slot_entry.length as usize;

        if end > page.data().len() {
            return Err(HeapError::TupleNotFound(tuple_id));
        }

        let tuple_data = &page.data()[start..end];
        let tuple: Tuple = bincode::deserialize(tuple_data)
            .map_err(|e| HeapError::Serialization(e.to_string()))?;

        Ok(tuple)
    }

    /// Scan all tuples in the heap file
    pub fn scan(&mut self) -> Result<Vec<(TupleId, Tuple)>, HeapError> {
        let mut results = Vec::new();
        let mut page_id = 0;

        // Scan pages until we hit an empty one
        loop {
            let page = match self.page_file.read_page(page_id) {
                Ok(p) => p,
                Err(_) => break,
            };

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

                    if let Ok(tuple) = self.read_tuple(tuple_id) {
                        results.push((tuple_id, tuple));
                    }
                }
            }

            page_id += 1;

            // Stop scanning after reasonable number of pages
            if page_id > 1000 {
                break;
            }
        }

        Ok(results)
    }

    /// Load all tuples into a Relation
    pub fn load_relation(&mut self) -> Result<Relation, HeapError> {
        let tuples: Vec<Tuple> = self.scan()?.into_iter().map(|(_, t)| t).collect();

        Relation::from_tuples(self.relation_type.clone(), tuples)
            .map_err(|e| HeapError::Serialization(e.to_string()))
    }

    /// Store a relation's tuples into the heap file
    pub fn store_relation(&mut self, relation: &Relation) -> Result<Vec<TupleId>, HeapError> {
        let mut tuple_ids = Vec::new();

        for tuple in relation.tuples() {
            let tuple_id = self.insert_tuple(tuple)?;
            tuple_ids.push(tuple_id);
        }

        Ok(tuple_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};
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
        let tuple_id = heap.insert_tuple(&tuple).unwrap();

        let read_tuple = heap.read_tuple(tuple_id).unwrap();
        assert_eq!(read_tuple, tuple);
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

        let tid1 = heap.insert_tuple(&tuple1).unwrap();
        let tid2 = heap.insert_tuple(&tuple2).unwrap();
        let tid3 = heap.insert_tuple(&tuple3).unwrap();

        assert_eq!(heap.read_tuple(tid1).unwrap(), tuple1);
        assert_eq!(heap.read_tuple(tid2).unwrap(), tuple2);
        assert_eq!(heap.read_tuple(tid3).unwrap(), tuple3);
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

    #[test]
    fn test_tuple_not_found() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();
        let mut heap = HeapFile::create(path, rel_type.clone()).unwrap();

        let invalid_id = TupleId {
            page_id: 999,
            slot: 0,
        };

        let result = heap.read_tuple(invalid_id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HeapError::TupleNotFound(_)));
    }
}
