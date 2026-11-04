use crate::persistent_engine::*;
use relvar_core::types::{ScalarType, TupleType};
use relvar_storage_core::slotted::{PAGE_FORMAT_VERSION, VersionedSlotEntry, VersionedSlottedPage};
use std::path::Path;

pub fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

/// Decodes every version slot of `<dir>/<relation>.heap` straight from the
/// file, so tests can observe the physical heap (xmin/xmax marks, reclaimed
/// slots) without touching private engine state.
///
/// Heap pages are `PAGE_SIZE` blocks framed as `[u64 len][page data]`;
/// versioned page data starts with the v2 header
/// `[format version][u32 slot-dir len]`.
pub fn heap_version_slots(dir: &Path, relation: &str) -> Vec<VersionedSlotEntry> {
    let bytes = std::fs::read(dir.join(format!("{relation}.heap"))).unwrap();
    let mut slots = Vec::new();
    for block in bytes.chunks(crate::storage::PAGE_SIZE) {
        if block.len() < 8 {
            break;
        }
        let len = u64::from_le_bytes(block[0..8].try_into().unwrap()) as usize;
        if len == 0 {
            break;
        }
        let data = &block[8..8 + len];
        assert_eq!(data[0], PAGE_FORMAT_VERSION);
        let slot_dir_len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
        let page: VersionedSlottedPage = postcard::from_bytes(&data[5..5 + slot_dir_len]).unwrap();
        slots.extend(page.slots.into_iter().flatten());
    }
    slots
}
