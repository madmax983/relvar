use super::*;

impl HeapFile {
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
    pub(crate) fn check_tuple_size_limit(&self, tuple_data_len: usize) -> Result<(), HeapError> {
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

    /// Helper to find a free slot or allocate a new one.
    pub(crate) fn find_or_allocate_slot<T>(slots: &mut Vec<Option<T>>, slot_count: &mut u32) -> u32 {
        if let Some(pos) = slots.iter().position(|s| s.is_none()) {
            pos as u32
        } else {
            let new_slot = slots.len() as u32;
            slots.push(None);
            *slot_count += 1;
            new_slot
        }
    }

    /// Finds the first page that can accommodate the insertion.
    /// Retries on PageFull error by incrementing the page ID.
    pub(crate) fn find_page_for_insertion<F, R>(&mut self, mut insert_fn: F) -> Result<R, HeapError>
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
    pub(crate) fn try_insert_into_page(
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

        let slot_number =
            Self::prepare_insert(&mut slotted_page, &mut existing_tuples, tuple_data)?;

        // Serialize the updated page with all tuples
        let page_data = self.serialize_slotted_page_with_tuples(&slotted_page, &existing_tuples)?;

        // Write the page
        let updated_page = Page::from_data(page_id, page_data)?;
        self.page_file.write_page(&updated_page)?;

        Ok(slot_number)
    }

    pub(crate) fn prepare_insert(
        slotted_page: &mut SlottedPage,
        existing_tuples: &mut Vec<Vec<u8>>,
        tuple_data: &[u8],
    ) -> Result<u32, HeapError> {
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
        let required_space = header_size
            .checked_add(total_tuple_data_size)
            .ok_or_else(|| {
                HeapError::Serialization("Header size + total tuple data size overflow".to_string())
            })?;

        if required_space > USABLE_PAGE_SIZE_V1 {
            return Err(HeapError::PageFull);
        }

        // Repack slots
        Self::repack_slots(
            &mut slotted_page.slots,
            existing_tuples,
            USABLE_PAGE_SIZE_V1,
        )?;

        Ok(slot_number)
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
    pub(crate) fn check_versioned_tuple_size_limit(
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
    pub(crate) fn try_insert_into_page_versioned(
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

        let slot_number = Self::prepare_insert_versioned(
            &mut versioned_page,
            &mut existing_tuples,
            tuple_data,
            txn_id,
            prev_version,
        )?;

        // Serialize the updated page with all tuples
        let page_data =
            self.serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)?;

        // Write the page
        let updated_page = Page::from_data(page_id, page_data)?;
        self.page_file.write_page(&updated_page)?;

        Ok(slot_number)
    }

    pub(crate) fn prepare_insert_versioned(
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

}
