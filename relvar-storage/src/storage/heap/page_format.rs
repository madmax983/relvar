use super::TupleId;
use crate::storage::page::PAGE_SIZE;
use serde::{Deserialize, Serialize};

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
