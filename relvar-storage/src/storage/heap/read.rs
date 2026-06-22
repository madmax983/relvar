use super::*;

impl HeapFile {
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
