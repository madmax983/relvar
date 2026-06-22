use super::*;

impl HeapFile {
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

}
