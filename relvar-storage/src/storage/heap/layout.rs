use super::super::page::{PAGE_SIZE, PageId};
use crate::storage::heap::HeapError;
use serde::{Deserialize, Serialize};

/// Helper for bounded deserialization to prevent allocation bombs.
/// Limits allocation size to PAGE_SIZE (4KB), preventing DoS from malicious length prefixes.
pub(crate) fn deserialize_bounded<'a, T>(data: &'a [u8]) -> Result<T, HeapError>
where
    T: Deserialize<'a>,
{
    postcard::from_bytes(data).map_err(|e| HeapError::Serialization(e.to_string()))
}

/// Helper for consistent serialization matching `deserialize_bounded`.
pub(crate) fn serialize_compat<T: Serialize>(value: &T) -> Result<Vec<u8>, HeapError> {
    postcard::to_allocvec(value).map_err(|e| HeapError::Serialization(e.to_string()))
}

/// Helper for calculating serialized size matching `serialize_compat`.
pub(crate) fn serialized_size_compat<T: Serialize>(value: &T) -> Result<u64, HeapError> {
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

/// Slot directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SlotEntry {
    pub(crate) offset: u32,
    pub(crate) length: u32,
}

/// Versioned slot directory entry for MVCC.
///
/// Extends SlotEntry with transaction version metadata to support
/// Multi-Version Concurrency Control (MVCC).
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

impl SlotEntry {
    pub(crate) fn offset(&self) -> u32 {
        self.offset
    }

    pub(crate) fn length(&self) -> u32 {
        self.length
    }

    pub(crate) fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    pub(crate) fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

impl VersionedSlotEntry {
    pub(crate) fn offset(&self) -> u32 {
        self.offset
    }

    pub(crate) fn length(&self) -> u32 {
        self.length
    }

    pub(crate) fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    pub(crate) fn set_length(&mut self, length: u32) {
        self.length = length;
    }
}

/// Page layout: `[slot_count (4 bytes)] [slot_entries...] [free_space] [...tuple_data]`
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

// Page format version to handle serialization changes
pub(crate) const PAGE_FORMAT_VERSION: u8 = 2; // Version 2: length-prefixed slot directory

pub(crate) const USABLE_PAGE_SIZE_V1: usize = PAGE_SIZE - 8;
pub(crate) const USABLE_PAGE_SIZE_V2: usize = PAGE_SIZE - 8;
pub(crate) const V2_HEADER_SIZE: usize = 5; // 1 byte version + 4 bytes length
