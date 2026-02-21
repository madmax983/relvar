use super::page::{PAGE_SIZE, Page, PageId};
use bincode::Options;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for slotted page operations.
#[derive(Debug, Error)]
pub enum SlottedPageError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Page full")]
    PageFull,
    #[error("Tuple too large: {0} bytes")]
    TupleTooLarge(usize),
}

/// Helper for bounded deserialization to prevent allocation bombs.
pub(crate) fn deserialize_bounded<'a, T>(data: &'a [u8]) -> Result<T, SlottedPageError>
where
    T: Deserialize<'a>,
{
    bincode::options()
        .with_little_endian()
        .with_fixint_encoding()
        .with_limit(PAGE_SIZE as u64)
        .allow_trailing_bytes()
        .deserialize(data)
        .map_err(|e| SlottedPageError::Serialization(e.to_string()))
}

/// Tuple ID: (page_id, slot_number)
/// Internal to storage layer only (TTM Proscription 6)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct TupleId {
    pub(crate) page_id: PageId,
    pub(crate) slot: u32,
}

/// Operation type for size checking (Insert vs Update)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationType {
    Insert,
    Update,
}

/// Slot directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SlotEntry {
    pub(crate) offset: u32,
    pub(crate) length: u32,
}

/// Versioned slot directory entry for MVCC.
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

/// Helper trait to abstract over SlotEntry and VersionedSlotEntry
pub(crate) trait SlotDescriptor {
    fn offset(&self) -> u32;
    fn length(&self) -> u32;
}

/// Helper trait to abstract over SlotEntry and VersionedSlotEntry modifications
pub(crate) trait MutableSlot: SlotDescriptor {
    fn set_offset(&mut self, offset: u32);
    fn set_length(&mut self, length: u32);
}

impl SlotDescriptor for SlotEntry {
    fn offset(&self) -> u32 {
        self.offset
    }

    fn length(&self) -> u32 {
        self.length
    }
}

impl MutableSlot for SlotEntry {
    fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

impl SlotDescriptor for VersionedSlotEntry {
    fn offset(&self) -> u32 {
        self.offset
    }

    fn length(&self) -> u32 {
        self.length
    }
}

impl MutableSlot for VersionedSlotEntry {
    fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

/// Page layout: [slot_count (4 bytes)] [slot_entries...] [free_space] [...tuple_data]
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
pub(crate) const PAGE_FORMAT_VERSION: u8 = 2; // Version 2: length-prefixed slot directory
pub(crate) const USABLE_PAGE_SIZE_V1: usize = PAGE_SIZE - 8;
pub(crate) const USABLE_PAGE_SIZE_V2: usize = PAGE_SIZE - 8;
pub(crate) const V2_HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length

impl SlottedPage {
    pub(crate) fn new() -> Self {
        Self {
            slot_count: 0,
            slots: Vec::new(),
        }
    }
}

impl VersionedSlottedPage {
    pub(crate) fn new() -> Self {
        Self {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 0,
            slots: Vec::new(),
        }
    }
}

/// Helper to repack slots and calculate offsets.
pub(crate) fn repack_slots<T: MutableSlot>(
    slots: &mut [Option<T>],
    tuples: &[Vec<u8>],
    usable_size: usize,
) -> Result<(), SlottedPageError> {
    let mut current_offset = usable_size;
    for (idx, tuple) in tuples.iter().enumerate().rev() {
        if tuple.is_empty() {
            continue;
        }

        if let Some(slot) = slots.get_mut(idx).and_then(|s| s.as_mut()) {
            current_offset = current_offset
                .checked_sub(tuple.len())
                .ok_or(SlottedPageError::PageFull)?;
            slot.set_offset(current_offset as u32);
            slot.set_length(tuple.len() as u32);
        }
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

/// Serialize a slotted page with all tuple data
pub(crate) fn serialize_slotted_page_with_tuples(
    slotted_page: &SlottedPage,
    tuples: &[Vec<u8>],
) -> Result<Vec<u8>, SlottedPageError> {
    // Serialize slot directory
    let slot_dir = bincode::serialize(slotted_page)
        .map_err(|e| SlottedPageError::Serialization(e.to_string()))?;

    // Create page buffer
    let mut data = vec![0u8; USABLE_PAGE_SIZE_V1];

    // Copy slot directory at beginning
    data[..slot_dir.len()].copy_from_slice(&slot_dir);

    // Copy each tuple at its designated offset
    for (idx, slot_entry) in slotted_page.slots.iter().enumerate() {
        if let Some(entry) = slot_entry {
            let offset = entry.offset as usize;
            let length = entry.length as usize;
            if idx < tuples.len() && !tuples[idx].is_empty() {
                if offset + length > data.len() {
                     return Err(SlottedPageError::Serialization(format!(
                        "Slot {} points outside buffer: offset={}, length={}, buffer_len={}",
                        idx, offset, length, data.len()
                    )));
                }
                data[offset..offset + length].copy_from_slice(&tuples[idx]);
            }
        }
    }

    Ok(data)
}

/// Serialize a versioned page with all tuple data
pub(crate) fn serialize_versioned_page_with_tuples(
    versioned_page: &VersionedSlottedPage,
    tuples: &[Vec<u8>],
) -> Result<Vec<u8>, SlottedPageError> {
    // Serialize slot directory
    let slot_dir = bincode::serialize(versioned_page)
        .map_err(|e| SlottedPageError::Serialization(e.to_string()))?;

    if slot_dir.len() > u32::MAX as usize {
        return Err(SlottedPageError::Serialization(
            "Slot directory too large to be represented by u32 length prefix".to_string(),
        ));
    }

    if slot_dir
        .len()
        .checked_add(V2_HEADER_SIZE)
        .ok_or_else(|| SlottedPageError::Serialization("Header size overflow".to_string()))?
        > USABLE_PAGE_SIZE_V2
    {
        return Err(SlottedPageError::Serialization(format!(
            "slot directory too large for page: {} > {}",
            slot_dir.len() + V2_HEADER_SIZE,
            USABLE_PAGE_SIZE_V2
        )));
    }

    // Create page buffer
    let mut data = vec![0u8; USABLE_PAGE_SIZE_V2];

    // Write format version
    data[0] = PAGE_FORMAT_VERSION;

    // Write slot directory length
    let slot_dir_len = slot_dir.len() as u32;
    data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

    // Copy slot directory after header
    data[V2_HEADER_SIZE..V2_HEADER_SIZE + slot_dir.len()].copy_from_slice(&slot_dir);

    // Copy each tuple at its designated offset
    for (idx, slot_entry) in versioned_page.slots.iter().enumerate() {
        if let Some(entry) = slot_entry {
            let offset = entry.offset as usize;
            let length = entry.length as usize;
            if idx < tuples.len() && !tuples[idx].is_empty() {
                // Validate that offset + length doesn't exceed buffer
                if offset + length > data.len() {
                    return Err(SlottedPageError::Serialization(format!(
                        "Slot {} points outside buffer: offset={}, length={}, buffer_len={}",
                        idx,
                        offset,
                        length,
                        data.len()
                    )));
                }
                data[offset..offset + length].copy_from_slice(&tuples[idx]);
            }
        }
    }

    Ok(data)
}

/// Check if a tuple can theoretically fit in an empty page
pub(crate) fn check_tuple_size_limit(tuple_data_len: usize) -> Result<(), SlottedPageError> {
    // Create a dummy page with one slot to calculate exact header size
    let dummy_page = SlottedPage {
        slot_count: 1,
        slots: vec![Some(SlotEntry {
            offset: 0,
            length: tuple_data_len as u32,
        })],
    };

    let header_size = bincode::serialized_size(&dummy_page)
        .map_err(|e| SlottedPageError::Serialization(e.to_string()))?
        as usize;

    if header_size + tuple_data_len > USABLE_PAGE_SIZE_V1 {
        return Err(SlottedPageError::TupleTooLarge(tuple_data_len));
    }
    Ok(())
}

/// Check if a versioned tuple can theoretically fit in an empty page
pub(crate) fn check_versioned_tuple_size_limit(
    tuple_data_len: usize,
    op_type: OperationType,
) -> Result<(), SlottedPageError> {
    // Create a dummy versioned page with one slot
    let dummy_page = VersionedSlottedPage {
        magic: VERSIONED_PAGE_MAGIC,
        slot_count: 1,
        slots: vec![Some(VersionedSlotEntry {
            offset: 0,
            length: tuple_data_len as u32,
            xmin: crate::wal::TransactionId::new(0),
            xmax: None,
            prev_version: match op_type {
                OperationType::Update => Some(TupleId {
                    page_id: 0,
                    slot: 0,
                }),
                OperationType::Insert => None,
            },
        })],
    };

    let header_size = bincode::serialized_size(&dummy_page)
        .map_err(|e| SlottedPageError::Serialization(e.to_string()))?
        as usize;

    if V2_HEADER_SIZE + header_size + tuple_data_len > USABLE_PAGE_SIZE_V2 {
        return Err(SlottedPageError::TupleTooLarge(tuple_data_len));
    }
    Ok(())
}

/// Checks if a page is a versioned page (MVCC).
pub(crate) fn is_versioned_page(page: &Page) -> bool {
    // Check for new format: version byte + magic at offset 5
    let is_new_format = page.data().len() >= 9 && page.data()[0] == PAGE_FORMAT_VERSION && {
        let magic_at_5 = u32::from_le_bytes([
            page.data()[5],
            page.data()[6],
            page.data()[7],
            page.data()[8],
        ]);
        magic_at_5 == VERSIONED_PAGE_MAGIC
    };

    // Check for old format versioned: magic at offset 0
    let is_old_versioned = !is_new_format && page.data().len() >= 4 && {
        let first_u32 = u32::from_le_bytes([
            page.data()[0],
            page.data()[1],
            page.data()[2],
            page.data()[3],
        ]);
        first_u32 == VERSIONED_PAGE_MAGIC
    };

    is_new_format || is_old_versioned
}

/// Deserializes a versioned page, handling both V1 and V2 formats.
pub(crate) fn deserialize_versioned_page(page: &Page) -> Result<VersionedSlottedPage, SlottedPageError> {
    if !page.data().is_empty() && page.data()[0] == PAGE_FORMAT_VERSION {
        if page.data().len() < 5 {
            return Err(SlottedPageError::Serialization(
                "Versioned page too short to contain header".to_string(),
            ));
        }
        // New format: [version:1][length:4][slot_dir][tuples]
        let len_bytes: [u8; 4] = page.data()[1..5].try_into().map_err(|_| {
            SlottedPageError::Serialization(
                "Invalid slot directory length prefix in versioned page header".to_string(),
            )
        })?;
        let slot_dir_len = u32::from_le_bytes(len_bytes) as usize;

        // Ensure the declared slot directory length fits within the page data
        // Header is 5 bytes (1 byte version + 4 bytes length)
        let end_of_header = 5usize.checked_add(slot_dir_len).ok_or_else(|| {
            SlottedPageError::Serialization("Slot directory length overflow".to_string())
        })?;

        if end_of_header > page.data().len() {
            return Err(SlottedPageError::Serialization(format!(
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
pub(crate) fn deserialize_slotted_page(page: &Page) -> Result<SlottedPage, SlottedPageError> {
    deserialize_bounded(page.data())
}
