//! Slotted-page layouts for heap pages.
//!
//! Moved from `relvar-storage`'s `storage/heap/mod.rs` into the `no_std`
//! storage core, adapting `std::` paths to `core::`/`alloc::` and the error
//! type. Only the page-*payload* layout moved here — the [`Page`](crate::page::Page)
//! framing (length header) and the [`BlockDevice`](crate::device::BlockDevice)
//! stay in their own modules; the heap file itself stays on the host side
//! for the rewire worker.
//!
//! # Page payload layouts
//!
//! Two payload formats exist, distinguished by [`is_versioned_page`]:
//!
//! * **v1 (legacy):** `[postcard slot directory][tuple data]`. The slot
//!   directory is the postcard encoding of [`SlottedPage`]; tuple bytes
//!   grow downward from the end of the usable page area.
//! * **v2 (MVCC):** `[version: 1 byte][slot_dir_len: 4 bytes LE][postcard slot
//!   directory][tuple data]`, where the slot directory is the postcard
//!   encoding of [`VersionedSlottedPage`] and `version` is
//!   [`PAGE_FORMAT_VERSION`]. Very old versioned pages without the 5-byte
//!   header (bare postcard) still decode via the legacy path.
//!
//! All framing here is byte-identical to what the host heap file has always
//! written: same structs + same postcard version = same bytes.
//!
//! # TTM Compliance
//!
//! [`TupleId`] is storage-internal (TTM Proscription 6): it never crosses
//! into the logical relational layer.

use crate::page::PAGE_SIZE;
use crate::wal::TransactionId;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Helper for bounded deserialization to prevent allocation bombs.
///
/// postcard reads `Vec`s/`String`s out of the input buffer, so allocations
/// can never exceed the input length; the `SlottedError::Serialization`
/// mapping keeps a single core-only error type.
fn deserialize_bounded<'a, T>(data: &'a [u8]) -> Result<T, SlottedError>
where
    T: Deserialize<'a>,
{
    postcard::from_bytes(data).map_err(|e| SlottedError::Serialization(e.to_string()))
}

/// Helper for consistent serialization matching `deserialize_bounded`.
fn serialize_compat<T: Serialize>(value: &T) -> Result<Vec<u8>, SlottedError> {
    postcard::to_allocvec(value).map_err(|e| SlottedError::Serialization(e.to_string()))
}

/// Errors that can occur during slotted-page operations.
///
/// Core-only: no `std::io` variant — I/O failures belong to the
/// [`BlockDevice`](crate::device::BlockDevice) implementation.
#[derive(Debug, Error)]
pub enum SlottedError {
    /// Tuple serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The page has insufficient space for the tuple.
    #[error("Page full")]
    PageFull,

    /// The tuple is too large to fit in a page.
    #[error("Tuple too large: {0} bytes")]
    TupleTooLarge(usize),
}

/// Tuple ID: (page_id, slot_number).
///
/// Storage-internal only (TTM Proscription 6): physical addressing for the
/// storage engine. Never exposed to the logical relational layer, where
/// tuples are identified by their attribute values (keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TupleId {
    /// The page holding the tuple.
    pub page_id: crate::page::PageId,
    /// The slot within the page.
    pub slot: u32,
}

/// Slot directory entry: where one tuple's bytes live in the page payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotEntry {
    /// Byte offset of the tuple data within the page payload.
    pub offset: u32,
    /// Byte length of the tuple data.
    pub length: u32,
}

/// Versioned slot directory entry for MVCC.
///
/// Extends [`SlotEntry`] with transaction version metadata to support
/// Multi-Version Concurrency Control (MVCC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedSlotEntry {
    /// Byte offset of the tuple data within the page payload.
    pub offset: u32,
    /// Byte length of the tuple data.
    pub length: u32,
    /// Transaction that created this version.
    pub xmin: TransactionId,
    /// Transaction that deleted/updated this version (`None` = still visible).
    pub xmax: Option<TransactionId>,
    /// Previous version in the version chain (for undo).
    pub prev_version: Option<TupleId>,
}

/// Page layout: `[slot_count (4 bytes)] [slot_entries...] [free_space] [...tuple_data]`
///
/// This is the postcard-encoded slot directory at the start of a v1 page
/// payload. (The "4 bytes" is the postcard varint encoding of `slot_count`.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlottedPage {
    /// Number of slots (including freed ones).
    pub slot_count: u32,
    /// Slot directory; `None` entries are freed slots.
    pub slots: Vec<Option<SlotEntry>>,
}

/// Versioned page layout for MVCC.
///
/// Postcard-encoded slot directory of a v2 page payload, preceded on disk
/// by the 5-byte v2 header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedSlottedPage {
    /// Magic number distinguishing this from [`SlottedPage`]:
    /// [`VERSIONED_PAGE_MAGIC`] ("MVCC" in ASCII).
    pub magic: u32,
    /// Number of slots (including freed ones).
    pub slot_count: u32,
    /// Slot directory; `None` entries are freed slots.
    pub slots: Vec<Option<VersionedSlotEntry>>,
}

/// Magic number marking a versioned (MVCC) slot directory: "MVCC" in ASCII.
pub const VERSIONED_PAGE_MAGIC: u32 = 0x4D564343;

/// Page format version for the v2 length-prefixed slot directory layout.
pub const PAGE_FORMAT_VERSION: u8 = 2;

/// v2 header size: 1 byte version + 4 bytes slot-directory length.
pub const V2_HEADER_SIZE: usize = 5;

/// Usable page payload size for v1 pages (page size minus the 8-byte page
/// length header).
pub const USABLE_PAGE_SIZE_V1: usize = PAGE_SIZE - 8;

/// Usable page payload size for v2 pages (page size minus the 8-byte page
/// length header).
pub const USABLE_PAGE_SIZE_V2: usize = PAGE_SIZE - 8;

/// Checks whether a page payload holds a versioned (MVCC) slot directory.
///
/// Detects both the v2 format (version byte + length + magic at offset 5)
/// and the old bare-postcard format (magic at offset 0). postcard encodes
/// the `u32` magic `0x4D564343` as a varint: `[195, 134, 217, 234, 4]`.
pub fn is_versioned_page(payload: &[u8]) -> bool {
    // postcard uses varint for u32. 0x4D564343 encodes to [195, 134, 217, 234, 4].
    let postcard_magic = [195, 134, 217, 234, 4];

    // Check for new format: version byte + length (4 bytes) + magic at offset 5
    let is_new_format = payload.len() >= 5 + postcard_magic.len()
        && payload[0] == PAGE_FORMAT_VERSION
        && { payload[5..5 + postcard_magic.len()] == postcard_magic };

    // Check for old format (just the magic number at the start)
    let is_old_versioned = !is_new_format && payload.len() >= postcard_magic.len() && {
        payload[0..postcard_magic.len()] == postcard_magic
    };

    is_new_format || is_old_versioned
}

/// Encodes a v1 page payload: `[postcard slot directory][tuple data]`.
///
/// Builds the full [`USABLE_PAGE_SIZE_V1`]-byte payload with the postcard
/// slot directory at offset 0 and each tuple's bytes at its slot's offset.
/// This is the exact legacy layout the host heap file has always written.
///
/// # Errors
///
/// Returns [`SlottedError::Serialization`] if a slot points outside the
/// buffer, overlaps the slot-directory header, or if offsets overflow.
pub fn encode_slotted_page(
    page: &SlottedPage,
    tuples: &[Vec<u8>],
) -> Result<Vec<u8>, SlottedError> {
    // Serialize slot directory
    let slot_dir = serialize_compat(page)?;

    // Create page buffer
    let mut data = alloc::vec![0u8; USABLE_PAGE_SIZE_V1];

    // Copy slot directory at beginning
    data[..slot_dir.len()].copy_from_slice(&slot_dir);

    // Copy each tuple at its designated offset
    for (idx, slot_entry) in page.slots.iter().enumerate() {
        if let Some(entry) = slot_entry {
            let offset = entry.offset as usize;
            let length = entry.length as usize;
            if idx < tuples.len() && !tuples[idx].is_empty() {
                let end_offset = offset.checked_add(length).ok_or_else(|| {
                    SlottedError::Serialization(String::from("Tuple offset + length overflow"))
                })?;
                let header_end = slot_dir.len();
                if end_offset > data.len() || offset < header_end {
                    return Err(SlottedError::Serialization(format!(
                        "Slot points outside buffer or overlaps header: offset={}, length={}, buffer_len={}, header_end={}",
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

    Ok(data)
}

/// Decodes a v1 page payload's slot directory.
///
/// The payload is `[postcard slot directory][tuple data]`; only the
/// directory is deserialized here (tuple bytes are never read).
pub fn decode_slotted_page(payload: &[u8]) -> Result<SlottedPage, SlottedError> {
    deserialize_bounded(payload)
}

/// Encodes a v2 page payload: `[version:1][slot_dir_len:4 LE][postcard slot directory][tuple data]`.
///
/// Builds the full [`USABLE_PAGE_SIZE_V2`]-byte payload. This is the exact
/// framing the host heap file's `serialize_versioned_page_with_tuples`
/// has always written.
///
/// # Errors
///
/// Returns [`SlottedError::Serialization`] if the slot directory does not
/// fit, if a slot points outside the buffer or overlaps the header, or if
/// offsets overflow.
pub fn encode_versioned_page(
    page: &VersionedSlottedPage,
    tuples: &[Vec<u8>],
) -> Result<Vec<u8>, SlottedError> {
    // Serialize slot directory
    let slot_dir = serialize_compat(page)?;

    // Format: [version:1 byte][slot_dir_length:4 bytes][slot_dir][tuple_data]
    const HEADER_SIZE: usize = V2_HEADER_SIZE;
    const USABLE_PAGE_SIZE: usize = USABLE_PAGE_SIZE_V2;

    verify_versioned_page_size(&slot_dir, HEADER_SIZE, USABLE_PAGE_SIZE)?;

    // Create page buffer
    let mut data = alloc::vec![0u8; USABLE_PAGE_SIZE];

    // Write format version
    data[0] = PAGE_FORMAT_VERSION;

    // Write slot directory length
    let slot_dir_len = slot_dir.len() as u32;
    data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());

    // Copy slot directory after header
    data[HEADER_SIZE..HEADER_SIZE + slot_dir.len()].copy_from_slice(&slot_dir);

    copy_versioned_tuples_to_buffer(&mut data, page, tuples, HEADER_SIZE, slot_dir.len())?;

    Ok(data)
}

fn verify_versioned_page_size(
    slot_dir: &[u8],
    header_size: usize,
    usable_size: usize,
) -> Result<(), SlottedError> {
    if slot_dir.len() > u32::MAX as usize {
        return Err(SlottedError::Serialization(String::from(
            "Slot directory too large to be represented by u32 length prefix",
        )));
    }

    if slot_dir
        .len()
        .checked_add(header_size)
        .ok_or_else(|| SlottedError::Serialization(String::from("Header size overflow")))?
        > usable_size
    {
        return Err(SlottedError::Serialization(format!(
            "slot directory too large for page: {} > {}",
            slot_dir.len() + header_size,
            usable_size
        )));
    }
    Ok(())
}

fn copy_versioned_tuples_to_buffer(
    data: &mut [u8],
    versioned_page: &VersionedSlottedPage,
    tuples: &[Vec<u8>],
    header_size: usize,
    slot_dir_len: usize,
) -> Result<(), SlottedError> {
    for (idx, slot_entry) in versioned_page.slots.iter().enumerate() {
        if let Some(entry) = slot_entry {
            let offset = entry.offset as usize;
            let length = entry.length as usize;
            if idx < tuples.len() && !tuples[idx].is_empty() {
                let end_offset = offset.checked_add(length).ok_or_else(|| {
                    SlottedError::Serialization(String::from("Tuple offset + length overflow"))
                })?;
                let header_end = header_size.checked_add(slot_dir_len).ok_or_else(|| {
                    SlottedError::Serialization(String::from(
                        "Header size + slot directory length overflow",
                    ))
                })?;
                // Validate that offset + length doesn't exceed buffer and doesn't overlap header
                if end_offset > data.len() || offset < header_end {
                    return Err(SlottedError::Serialization(format!(
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

/// Decodes a versioned page payload's slot directory, handling v1 and v2.
///
/// * v2: `[version:1][slot_dir_len:4 LE][postcard slot directory][tuple data]`
/// * v1 (legacy): bare `[postcard slot directory][tuple data]`
///
/// Only the directory is deserialized; tuple bytes are never read.
///
/// # Errors
///
/// Returns [`SlottedError::Serialization`] if the header is truncated, the
/// declared slot-directory length exceeds the payload, or postcard
/// deserialization fails.
pub fn decode_versioned_page(payload: &[u8]) -> Result<VersionedSlottedPage, SlottedError> {
    if !payload.is_empty() && payload[0] == PAGE_FORMAT_VERSION {
        if payload.len() < V2_HEADER_SIZE {
            return Err(SlottedError::Serialization(String::from(
                "Versioned page too short to contain header",
            )));
        }
        // New format: [version:1][length:4][slot_dir][tuples]
        let len_bytes: [u8; 4] = payload[1..5].try_into().map_err(|_| {
            SlottedError::Serialization(String::from(
                "Invalid slot directory length prefix in versioned page header",
            ))
        })?;
        let slot_dir_len = u32::from_le_bytes(len_bytes) as usize;

        // Ensure the declared slot directory length fits within the payload.
        // Header is 5 bytes (1 byte version + 4 bytes length)
        let end_of_header = V2_HEADER_SIZE.checked_add(slot_dir_len).ok_or_else(|| {
            SlottedError::Serialization(String::from("Slot directory length overflow"))
        })?;

        if end_of_header > payload.len() {
            return Err(SlottedError::Serialization(format!(
                "Slot directory length ({}) exceeds page size",
                slot_dir_len
            )));
        }

        deserialize_bounded(&payload[V2_HEADER_SIZE..end_of_header])
    } else {
        // Old format: [slot_dir][tuples]
        deserialize_bounded(payload)
    }
}

/// Returns the tuple bytes a slot points to, bounds-checked.
///
/// Mirrors the host heap file's slot validation: the `[offset, offset+length)`
/// range must lie inside `payload`.
///
/// # Errors
///
/// Returns [`SlottedError::Serialization`] if the range overflows or points
/// outside the payload.
pub fn slot_bytes(payload: &[u8], offset: u32, length: u32) -> Result<&[u8], SlottedError> {
    let start = offset as usize;
    let end = start
        .checked_add(length as usize)
        .ok_or_else(|| SlottedError::Serialization(String::from("Tuple end offset overflow")))?;

    if end > payload.len() {
        return Err(SlottedError::Serialization(format!(
            "Corrupted slot points outside page data: offset={offset}, length={length}"
        )));
    }
    Ok(&payload[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn sample_tuples() -> Vec<Vec<u8>> {
        vec![vec![1, 2, 3, 4, 5], vec![6, 7, 8], vec![9, 10, 11, 12]]
    }

    /// Packs tuple offsets downward from the end of the usable area, the way
    /// the host heap file lays them out.
    fn pack_offsets(tuples: &[Vec<u8>], usable: usize) -> Vec<u32> {
        let mut offsets = vec![0u32; tuples.len()];
        let mut cursor = usable;
        for (idx, tuple) in tuples.iter().enumerate().rev() {
            cursor -= tuple.len();
            offsets[idx] = cursor as u32;
        }
        offsets
    }

    fn sample_slotted_page(tuples: &[Vec<u8>]) -> SlottedPage {
        let offsets = pack_offsets(tuples, USABLE_PAGE_SIZE_V1);
        SlottedPage {
            slot_count: tuples.len() as u32,
            slots: tuples
                .iter()
                .enumerate()
                .map(|(idx, tuple)| {
                    Some(SlotEntry {
                        offset: offsets[idx],
                        length: tuple.len() as u32,
                    })
                })
                .collect(),
        }
    }

    fn sample_versioned_page(tuples: &[Vec<u8>]) -> VersionedSlottedPage {
        let offsets = pack_offsets(tuples, USABLE_PAGE_SIZE_V2);
        VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: tuples.len() as u32,
            slots: tuples
                .iter()
                .enumerate()
                .map(|(idx, tuple)| {
                    Some(VersionedSlotEntry {
                        offset: offsets[idx],
                        length: tuple.len() as u32,
                        xmin: TransactionId::new(100 + idx as u64),
                        xmax: None,
                        prev_version: None,
                    })
                })
                .collect(),
        }
    }

    // ---- v1 ----

    #[test]
    fn test_v1_encode_decode_roundtrip() {
        let tuples = sample_tuples();
        let page = sample_slotted_page(&tuples);

        let payload = encode_slotted_page(&page, &tuples).unwrap();
        assert_eq!(payload.len(), USABLE_PAGE_SIZE_V1);

        let decoded = decode_slotted_page(&payload).unwrap();
        assert_eq!(decoded, page);

        // Tuple bytes land at the slot offsets.
        for (slot, tuple) in decoded.slots.iter().flatten().zip(tuples.iter()) {
            assert_eq!(
                slot_bytes(&payload, slot.offset, slot.length).unwrap(),
                tuple
            );
        }
    }

    #[test]
    fn test_v1_slot_dir_at_offset_zero() {
        let tuples = sample_tuples();
        let page = sample_slotted_page(&tuples);
        let payload = encode_slotted_page(&page, &tuples).unwrap();

        // The postcard slot directory starts the payload (legacy layout).
        let expected_dir = postcard::to_allocvec(&page).unwrap();
        assert_eq!(&payload[..expected_dir.len()], expected_dir.as_slice());
    }

    #[test]
    fn test_v1_decode_rejects_garbage() {
        let result = decode_slotted_page(&[0xFF; 64]);
        assert!(matches!(result, Err(SlottedError::Serialization(_))));
    }

    #[test]
    fn test_v1_encode_rejects_overlapping_slot() {
        let tuples = sample_tuples();
        let mut page = sample_slotted_page(&tuples);
        // Point slot 0 into the slot-directory header region.
        page.slots[0] = Some(SlotEntry {
            offset: 2,
            length: 10,
        });

        let result = encode_slotted_page(&page, &tuples);
        assert!(matches!(result, Err(SlottedError::Serialization(_))));
    }

    // ---- v2 ----

    #[test]
    fn test_v2_encode_decode_roundtrip() {
        let tuples = sample_tuples();
        let page = sample_versioned_page(&tuples);

        let payload = encode_versioned_page(&page, &tuples).unwrap();
        assert_eq!(payload.len(), USABLE_PAGE_SIZE_V2);

        let decoded = decode_versioned_page(&payload).unwrap();
        assert_eq!(decoded, page);
        assert_eq!(decoded.magic, VERSIONED_PAGE_MAGIC);

        for (slot, tuple) in decoded.slots.iter().flatten().zip(tuples.iter()) {
            assert_eq!(
                slot_bytes(&payload, slot.offset, slot.length).unwrap(),
                tuple
            );
        }
        // Version metadata survives the roundtrip.
        assert_eq!(
            decoded.slots[0].as_ref().unwrap().xmin,
            TransactionId::new(100)
        );
        assert_eq!(
            decoded.slots[1].as_ref().unwrap().xmin,
            TransactionId::new(101)
        );
        assert_eq!(decoded.slots[0].as_ref().unwrap().xmax, None);
    }

    #[test]
    fn test_v2_framing_is_exact() {
        let tuples = sample_tuples();
        let page = sample_versioned_page(&tuples);
        let payload = encode_versioned_page(&page, &tuples).unwrap();

        // [version:1][slot_dir_len:4 LE][postcard slot_dir][tuple data]
        assert_eq!(payload[0], PAGE_FORMAT_VERSION);
        let slot_dir_len = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
        let expected_dir = postcard::to_allocvec(&page).unwrap();
        assert_eq!(slot_dir_len, expected_dir.len());
        assert_eq!(
            &payload[V2_HEADER_SIZE..V2_HEADER_SIZE + slot_dir_len],
            expected_dir.as_slice()
        );
    }

    #[test]
    fn test_v2_decode_legacy_bare_postcard() {
        // Old versioned pages have no 5-byte header: bare postcard.
        let tuples = sample_tuples();
        let page = sample_versioned_page(&tuples);
        let bare = postcard::to_allocvec(&page).unwrap();
        assert_ne!(bare[0], PAGE_FORMAT_VERSION);

        let decoded = decode_versioned_page(&bare).unwrap();
        assert_eq!(decoded, page);
    }

    #[test]
    fn test_v2_decode_rejects_truncated_header() {
        let payload = [PAGE_FORMAT_VERSION, 1, 2];
        let result = decode_versioned_page(&payload);
        assert!(matches!(result, Err(SlottedError::Serialization(_))));
    }

    #[test]
    fn test_v2_decode_rejects_oversize_dir_len() {
        let mut payload = alloc::vec![0u8; 64];
        payload[0] = PAGE_FORMAT_VERSION;
        payload[1..5].copy_from_slice(&1_000_000u32.to_le_bytes());

        let result = decode_versioned_page(&payload);
        assert!(matches!(result, Err(SlottedError::Serialization(_))));
    }

    #[test]
    fn test_v2_encode_preserves_version_chain() {
        let tuples = sample_tuples();
        let mut page = sample_versioned_page(&tuples);
        // Link slot 1 back to slot 0 as its previous version, mark xmax.
        page.slots[1].as_mut().unwrap().xmax = Some(TransactionId::new(200));
        page.slots[1].as_mut().unwrap().prev_version = Some(TupleId {
            page_id: 7,
            slot: 0,
        });

        let payload = encode_versioned_page(&page, &tuples).unwrap();
        let decoded = decode_versioned_page(&payload).unwrap();
        let slot1 = decoded.slots[1].as_ref().unwrap();
        assert_eq!(slot1.xmax, Some(TransactionId::new(200)));
        assert_eq!(
            slot1.prev_version,
            Some(TupleId {
                page_id: 7,
                slot: 0
            })
        );
    }

    // ---- magic detection ----

    #[test]
    fn test_is_versioned_page() {
        let tuples = sample_tuples();

        let v2 = encode_versioned_page(&sample_versioned_page(&tuples), &tuples).unwrap();
        assert!(is_versioned_page(&v2));

        let bare = postcard::to_allocvec(&sample_versioned_page(&tuples)).unwrap();
        assert!(is_versioned_page(&bare));

        let v1 = encode_slotted_page(&sample_slotted_page(&tuples), &tuples).unwrap();
        assert!(!is_versioned_page(&v1));

        assert!(!is_versioned_page(&[]));
    }

    // ---- slot_bytes ----

    #[test]
    fn test_slot_bytes_bounds() {
        let payload = [10u8, 20, 30, 40, 50];

        assert_eq!(slot_bytes(&payload, 1, 3).unwrap(), &[20, 30, 40]);
        assert_eq!(slot_bytes(&payload, 0, 5).unwrap(), &payload);

        // Past the end.
        assert!(slot_bytes(&payload, 4, 2).is_err());
        // Overflow.
        assert!(slot_bytes(&payload, u32::MAX, 2).is_err());
        // Zero-length at the very end is fine.
        assert_eq!(slot_bytes(&payload, 5, 0).unwrap(), &[]);
    }

    // ---- constants ----

    #[test]
    fn test_layout_constants() {
        assert_eq!(VERSIONED_PAGE_MAGIC, 0x4D56_4343);
        assert_eq!(PAGE_FORMAT_VERSION, 2);
        assert_eq!(V2_HEADER_SIZE, 5);
        assert_eq!(USABLE_PAGE_SIZE_V1, PAGE_SIZE - 8);
        assert_eq!(USABLE_PAGE_SIZE_V2, PAGE_SIZE - 8);
    }

    #[test]
    fn test_tuple_id_is_copy() {
        let id = TupleId {
            page_id: 3,
            slot: 1,
        };
        let copy = id;
        assert_eq!(id, copy);
    }
}
