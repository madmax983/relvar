use super::*;

impl HeapFile {
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

            removed_count += self.gc_process_page(page_id, &page, oldest_active_lsn, committed)?;

            page_id += 1;
        }

        Ok(removed_count)
    }

    pub(crate) fn gc_process_page(
        &mut self,
        page_id: PageId,
        page: &Page,
        oldest_active_lsn: crate::wal::Lsn,
        committed: &std::collections::HashSet<crate::wal::TransactionId>,
    ) -> Result<usize, HeapError> {
        // Try to deserialize as versioned page
        let mut versioned_page = match self.deserialize_versioned_page(page) {
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

            let page_data =
                self.serialize_versioned_page_with_tuples(&versioned_page, &existing_tuples)?;
            let updated_page = Page::from_data(page_id, page_data)?;
            self.page_file.write_page(&updated_page)?;
        }

        Ok(removed_count)
    }

    pub(crate) fn is_dead_version(
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

}
