import re

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

# Replace trait usages with explicit implementations for SlotEntry and VersionedSlotEntry
# 1. repack_slots
content = content.replace(
'''    fn repack_slots<T: MutableSlot>(
        slots: &mut [Option<T>],
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
    }''',
'''    fn repack_slots(
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
                slot.offset = current_offset as u32;
                slot.length = tuple.len() as u32;
            }
        }
        Ok(())
    }

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
                slot.offset = current_offset as u32;
                slot.length = tuple.len() as u32;
            }
        }
        Ok(())
    }'''
)

# 2. extract_tuples_from_slots
content = content.replace(
'''    fn extract_tuples_from_slots<'a, I, S>(
        &self,
        page: &Page,
        slots: I,
    ) -> Result<Vec<Tuple>, HeapError>
    where
        I: Iterator<Item = &'a S>,
        S: SlotDescriptor + 'a,
    {
        let mut results = Vec::new();
        for slot in slots {
            let tuple = self.extract_tuple_from_page(page, slot.offset(), slot.length())?;
            results.push(tuple);
        }
        Ok(results)
    }''',
'''    fn extract_tuples_from_slots<'a, I>(
        &self,
        page: &Page,
        slots: I,
    ) -> Result<Vec<Tuple>, HeapError>
    where
        I: Iterator<Item = &'a SlotEntry>,
    {
        let mut results = Vec::new();
        for slot in slots {
            let tuple = self.extract_tuple_from_page(page, slot.offset, slot.length)?;
            results.push(tuple);
        }
        Ok(results)
    }

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
            let tuple = self.extract_tuple_from_page(page, slot.offset, slot.length)?;
            results.push(tuple);
        }
        Ok(results)
    }'''
)

# 3. extract_all_tuples
content = content.replace(
'''    fn extract_all_tuples<T: SlotDescriptor>(
        &self,
        page: &Page,
        slots: &[Option<T>],
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
    }''',
'''    fn extract_all_tuples(
        &self,
        page: &Page,
        slots: &[Option<SlotEntry>],
    ) -> Result<Vec<Vec<u8>>, HeapError> {
        let mut existing_tuples: Vec<Vec<u8>> = Vec::new();
        // Maintain alignment with slots: push empty Vec for None slots
        for slot_option in slots.iter() {
            if let Some(slot_entry) = slot_option {
                let raw_data =
                    self.extract_raw_tuple_data(page, slot_entry.offset, slot_entry.length)?;
                existing_tuples.push(raw_data);
            } else {
                existing_tuples.push(Vec::new());
            }
        }
        Ok(existing_tuples)
    }

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
                    self.extract_raw_tuple_data(page, slot_entry.offset, slot_entry.length)?;
                existing_tuples.push(raw_data);
            } else {
                existing_tuples.push(Vec::new());
            }
        }
        Ok(existing_tuples)
    }'''
)

# 4. Remove the traits and impls
content = re.sub(r'/// Helper trait to abstract over SlotEntry and VersionedSlotEntry\s*trait SlotDescriptor \{.*?\n\}\s*', '', content, flags=re.DOTALL)
content = re.sub(r'/// Helper trait to abstract over SlotEntry and VersionedSlotEntry modifications\s*trait MutableSlot: SlotDescriptor \{.*?\n\}\s*', '', content, flags=re.DOTALL)
content = re.sub(r'impl SlotDescriptor for SlotEntry \{.*?\n\}\s*', '', content, flags=re.DOTALL)
content = re.sub(r'impl MutableSlot for SlotEntry \{.*?\n\}\s*', '', content, flags=re.DOTALL)
content = re.sub(r'impl SlotDescriptor for VersionedSlotEntry \{.*?\n\}\s*', '', content, flags=re.DOTALL)
content = re.sub(r'impl MutableSlot for VersionedSlotEntry \{.*?\n\}\s*', '', content, flags=re.DOTALL)

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
