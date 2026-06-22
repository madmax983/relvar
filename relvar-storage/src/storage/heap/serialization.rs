use super::*;

impl HeapFile {
    /// Helper to repack slots and calculate offsets.
    /// Iterates backward from the end of the available space.
    /// Assumes slots are already populated (Some) for valid tuples.
    pub(crate) fn repack_slots(
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
    /// Computes the required header size (V2_HEADER_SIZE + slot directory length).
    /// Returns an error if the page would overflow.
    pub(crate) fn repack_and_verify_space(
        versioned_page: &mut VersionedSlottedPage,
        tuples: &[Vec<u8>],
        usable_size: usize,
    ) -> Result<usize, HeapError> {
        // 1. Repack slots. Since tuples grow downward from `usable_size` (which is constant),
        //    the assigned offsets are final and deterministic on the first pass.
        Self::repack_versioned_slots(&mut versioned_page.slots, tuples, usable_size)?;

        // 2. Serialize the updated slot directory. Now that the offsets are the large final values,
        //    the varint encoding will take its true maximum size.
        let slot_dir = serialize_compat(&versioned_page)?;
        let header_size = V2_HEADER_SIZE.checked_add(slot_dir.len()).ok_or_else(|| {
            HeapError::Serialization("Header size + slot directory length overflow".to_string())
        })?;

        // 3. Verify no overlap between the downward-growing tuples and the upward-growing header.
        for (idx, slot_entry) in versioned_page.slots.iter().enumerate() {
            if let Some(entry) = slot_entry {
                let offset = entry.offset as usize;
                if idx < tuples.len() && !tuples[idx].is_empty() && offset < header_size {
                    return Err(HeapError::PageFull);
                }
            }
        }

        Ok(header_size)
    }

    /// Helper to repack versioned slots and calculate offsets.
    /// Iterates backward from the end of the available space.
    /// Assumes slots are already populated (Some) for valid tuples.
    pub(crate) fn repack_versioned_slots(
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

    /// Helper to extract tuples from a sequence of slots.
    pub(crate) fn extract_tuples_from_slots<'a, I>(
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
            let tuple = self.extract_tuple_from_page(page, slot.offset(), slot.length())?;
            results.push(tuple);
        }
        Ok(results)
    }

    /// Helper to extract tuples from a sequence of versioned slots.
    pub(crate) fn extract_tuples_from_versioned_slots<'a, I>(
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
            let tuple = self.extract_tuple_from_page(page, slot.offset(), slot.length())?;
            results.push(tuple);
        }
        Ok(results)
    }

    /// Helper to extract all tuples from a page based on slot entries.
    /// This abstracts the common logic used in insert, update, delete, and GC operations.
    pub(crate) fn extract_all_tuples(
        &self,
        page: &Page,
        slots: &[Option<SlotEntry>],
    ) -> Result<Vec<Vec<u8>>, HeapError> {
        let mut existing_tuples: Vec<Vec<u8>> = Vec::with_capacity(slots.len());
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
    pub(crate) fn extract_all_versioned_tuples(
        &self,
        page: &Page,
        slots: &[Option<VersionedSlotEntry>],
    ) -> Result<Vec<Vec<u8>>, HeapError> {
        let mut existing_tuples: Vec<Vec<u8>> = Vec::with_capacity(slots.len());
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

    /// Serialize a slotted page with all tuple data
    pub(crate) fn serialize_slotted_page_with_tuples(
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

    /// Serialize a versioned page with all tuple data
    pub(crate) fn serialize_versioned_page_with_tuples(
        &self,
        versioned_page: &VersionedSlottedPage,
        tuples: &[Vec<u8>],
    ) -> Result<Vec<u8>, HeapError> {
        // Serialize slot directory
        let slot_dir = serialize_compat(versioned_page)?;

        // Format: [version:1 byte][slot_dir_length:4 bytes][slot_dir][tuple_data]
        const HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length
        const USABLE_PAGE_SIZE: usize = PAGE_SIZE - 8;

        Self::verify_versioned_page_size(&slot_dir, HEADER_SIZE, USABLE_PAGE_SIZE)?;

        // Create page buffer
        let mut data = vec![0u8; USABLE_PAGE_SIZE];

        // Write format version
        data[0] = PAGE_FORMAT_VERSION;

        // Write slot directory length
        let slot_dir_len = slot_dir.len() as u32;
        data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

        // Copy slot directory after header
        data[HEADER_SIZE..HEADER_SIZE + slot_dir.len()].copy_from_slice(&slot_dir);

        Self::copy_versioned_tuples_to_buffer(
            &mut data,
            versioned_page,
            tuples,
            HEADER_SIZE,
            slot_dir.len(),
        )?;

        Ok(data)
    }

    pub(crate) fn verify_versioned_page_size(
        slot_dir: &[u8],
        header_size: usize,
        usable_size: usize,
    ) -> Result<(), HeapError> {
        if slot_dir.len() > u32::MAX as usize {
            return Err(HeapError::Serialization(
                "Slot directory too large to be represented by u32 length prefix".to_string(),
            ));
        }

        if slot_dir
            .len()
            .checked_add(header_size)
            .ok_or_else(|| HeapError::Serialization("Header size overflow".to_string()))?
            > usable_size
        {
            return Err(HeapError::Serialization(format!(
                "slot directory too large for page: {} > {}",
                slot_dir.len() + header_size,
                usable_size
            )));
        }
        Ok(())
    }

    pub(crate) fn copy_versioned_tuples_to_buffer(
        data: &mut [u8],
        versioned_page: &VersionedSlottedPage,
        tuples: &[Vec<u8>],
        header_size: usize,
        slot_dir_len: usize,
    ) -> Result<(), HeapError> {
        for (idx, slot_entry) in versioned_page.slots.iter().enumerate() {
            if let Some(entry) = slot_entry {
                let offset = entry.offset as usize;
                let length = entry.length as usize;
                if idx < tuples.len() && !tuples[idx].is_empty() {
                    let end_offset = offset.checked_add(length).ok_or_else(|| {
                        HeapError::Serialization("Tuple offset + length overflow".to_string())
                    })?;
                    let header_end = header_size.checked_add(slot_dir_len).ok_or_else(|| {
                        HeapError::Serialization(
                            "Header size + slot directory length overflow".to_string(),
                        )
                    })?;
                    // Validate that offset + length doesn't exceed buffer and doesn't overlap header
                    if end_offset > data.len() || offset < header_end {
                        return Err(HeapError::Serialization(format!(
                            "Slot {} points outside buffer or overlaps header: offset={}, length={}, buffer_len={}, header_end={}",
                            idx,
                            offset,
                            length,
                            data.len(),
                            header_end
                        )));
                    }
                    data[offset..end_offset].copy_from_slice(&tuples[idx]);
                }
            }
        }
        Ok(())
    }

    /// Checks if a page is a versioned page (MVCC).
    pub(crate) fn is_versioned_page(&self, page: &Page) -> bool {
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
    pub(crate) fn deserialize_versioned_page(&self, page: &Page) -> Result<VersionedSlottedPage, HeapError> {
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
    pub(crate) fn deserialize_slotted_page(&self, page: &Page) -> Result<SlottedPage, HeapError> {
        deserialize_bounded(page.data())
    }

    /// Validates that a slot points to valid data within the page.
    pub(crate) fn validate_slot_bounds(
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
    pub(crate) fn extract_tuple_from_page(
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
    pub(crate) fn extract_raw_tuple_data(
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

}
