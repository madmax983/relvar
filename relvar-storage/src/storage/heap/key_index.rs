//! In-memory index enforcing a relation's implicit key across versions.
//!
//! TTM: RM Proscription 3 (no duplicate tuples). Every relation's whole
//! heading is a candidate key, so under MVCC each logical tuple must have
//! a single version chain: an insert of a tuple that already has a
//! version must either be a no-op (the version is visible) or a write
//! conflict (a concurrent transaction created it). Deciding that needs the
//! versions of one tuple value, which a heap scan finds only in O(n).
//!
//! [`KeyIndex`] maps a 64-bit digest of the tuple value to the internal
//! addresses ([`TupleId`]) of every live version slot holding a tuple with
//! that digest. Lookups read the candidate slots and compare the tuple
//! values, so digest collisions are harmless.
//!
//! # Lifecycle
//!
//! The index belongs to one open [`HeapFile`](super::HeapFile) and is
//! never persisted:
//!
//! - it is built lazily, with one heap scan, by the first lookup
//!   ([`KeyIndexState::Unbuilt`] until then);
//! - every versioned insert adds the new slot; GC and recovery undo
//!   remove the slots they reclaim; `xmax` changes need nothing (the
//!   index holds addresses, the lookup reads `xmin`/`xmax` from the page);
//! - a non-versioned rewrite, or a heap mutation that fails part-way,
//!   resets it to `Unbuilt`, so the next lookup rebuilds it from the heap
//!   instead of trusting a possibly incomplete index;
//! - dropping or re-creating the relation, and reopening the engine,
//!   discard it with the heap file object.
//!
//! The index only ever needs to be a *superset* of the live slots for
//! correctness (a stale address is filtered out when its slot is read);
//! a *missing* address would let a duplicate version through. The
//! test-only [`HeapFile::check_key_index`](super::HeapFile::check_key_index)
//! compares the index with a full scan exactly, and debug builds assert
//! that a candidate slot always still holds a tuple with its digest.
//!
//! # Memory bound
//!
//! An indexed version costs one `(u64 digest, u64 packed address)` pair:
//! 16 bytes of payload plus one hash-table control byte, so about 20–40
//! bytes per version once the table's 7/8 load factor and power-of-two
//! growth are counted. A digest shared by several versions (one tuple's
//! version chain) moves to a side table of small vectors. The index of one
//! relation holds at most [`DEFAULT_KEY_INDEX_CAPACITY`] (2^20) versions,
//! about 34 MiB in the worst case; past that the relation switches to
//! [`KeyIndexState::Overflow`] and lookups fall back to a full heap scan
//! (correct, but O(n) per insert). The next GC pass or engine reopen
//! retries the build.

use super::HeapFile;
use crate::mvcc::{VersionMetadata, VersionPool};
use crate::storage::heap::HeapError;
use foldhash::fast::RandomState;
use relvar_core::values::Tuple;
use relvar_storage_core::device::BlockDevice;
use relvar_storage_core::slotted::{
    TupleId, decode_versioned_page_into, is_versioned_page, slot_bytes,
};
use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault, Hasher};

/// Maximum number of versions the implicit-key index of one relation
/// holds before lookups fall back to heap scans (see the module docs).
pub(crate) const DEFAULT_KEY_INDEX_CAPACITY: usize = 1 << 20;

/// Bits of a packed address that hold the slot number. A 4 KiB page holds
/// far fewer than 2^16 slots.
const SLOT_BITS: u32 = 16;

/// Hasher for keys that are already uniformly distributed digests: the
/// digest is used as the table hash as-is, so a lookup hashes the tuple
/// exactly once.
#[derive(Default)]
struct PrehashedHasher(u64);

impl Hasher for PrehashedHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // Only `write_u64` is used by the index's `u64` keys; this keeps the
        // hasher total for any other key shape.
        for &byte in bytes {
            self.0 = self.0.rotate_left(8) ^ u64::from(byte);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

type Prehashed = BuildHasherDefault<PrehashedHasher>;

/// Packs an address into one `u64`: page id in the high 48 bits, slot in
/// the low 16. `None` for an address outside that range (a heap of more
/// than 2^48 pages), which the caller treats as index overflow.
fn pack(tuple_id: TupleId) -> Option<u64> {
    if tuple_id.slot >= 1 << SLOT_BITS || tuple_id.page_id >= 1 << (64 - SLOT_BITS) {
        return None;
    }
    Some((tuple_id.page_id << SLOT_BITS) | u64::from(tuple_id.slot))
}

/// Inverse of [`pack`].
fn unpack(packed: u64) -> TupleId {
    TupleId {
        page_id: packed >> SLOT_BITS,
        // Masked to 16 bits, so the cast is lossless.
        slot: (packed & ((1 << SLOT_BITS) - 1)) as u32,
    }
}

/// Digest → version addresses, for one relation (see the module docs).
pub(crate) struct KeyIndex {
    /// Per-index random seed (foldhash): digests are unpredictable, so
    /// crafted tuples cannot cheaply degrade lookups into collision chains,
    /// and hashing stays a small fraction of an insert's cost.
    seed: RandomState,
    /// Digests held by exactly one version slot (the common case).
    single: HashMap<u64, u64, Prehashed>,
    /// Digests held by two or more version slots: a tuple's version chain,
    /// or a digest collision.
    multi: HashMap<u64, Vec<u64>, Prehashed>,
    /// Number of indexed version slots.
    len: usize,
}

impl KeyIndex {
    /// An empty index with a fresh random seed.
    pub(crate) fn new() -> Self {
        Self {
            seed: RandomState::default(),
            single: HashMap::default(),
            multi: HashMap::default(),
            len: 0,
        }
    }

    /// The digest of a tuple value under this index's seed. Equal tuples
    /// have equal digests (`Tuple`'s `Hash` agrees with its `Eq`).
    pub(crate) fn digest(&self, tuple: &Tuple) -> u64 {
        self.seed.hash_one(tuple)
    }

    /// Number of indexed version slots.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Records that the slot at `tuple_id` holds a tuple with `digest`.
    ///
    /// Returns `false` (index unchanged) when the address cannot be packed;
    /// the caller must then stop trusting the index.
    pub(crate) fn insert(&mut self, digest: u64, tuple_id: TupleId) -> bool {
        let Some(packed) = pack(tuple_id) else {
            return false;
        };
        if let Some(ids) = self.multi.get_mut(&digest) {
            if !ids.contains(&packed) {
                ids.push(packed);
                self.len += 1;
            }
            return true;
        }
        match self.single.get(&digest) {
            Some(&existing) if existing == packed => {}
            Some(&existing) => {
                self.single.remove(&digest);
                self.multi.insert(digest, vec![existing, packed]);
                self.len += 1;
            }
            None => {
                self.single.insert(digest, packed);
                self.len += 1;
            }
        }
        true
    }

    /// Forgets the slot at `tuple_id` (holding a tuple with `digest`).
    /// Returns whether it was indexed.
    pub(crate) fn remove(&mut self, digest: u64, tuple_id: TupleId) -> bool {
        let Some(packed) = pack(tuple_id) else {
            return false;
        };
        if self.single.get(&digest) == Some(&packed) {
            self.single.remove(&digest);
            self.len -= 1;
            return true;
        }
        let Some(ids) = self.multi.get_mut(&digest) else {
            return false;
        };
        let Some(position) = ids.iter().position(|&id| id == packed) else {
            return false;
        };
        ids.swap_remove(position);
        self.len -= 1;
        if let [last] = ids.as_slice() {
            let last = *last;
            self.multi.remove(&digest);
            self.single.insert(digest, last);
        }
        true
    }

    /// Appends the addresses of every slot indexed under `digest` to `out`.
    pub(crate) fn candidates(&self, digest: u64, out: &mut Vec<TupleId>) {
        if let Some(&packed) = self.single.get(&digest) {
            out.push(unpack(packed));
        } else if let Some(ids) = self.multi.get(&digest) {
            out.extend(ids.iter().map(|&packed| unpack(packed)));
        }
    }

    /// Every `(digest, address)` pair, sorted (consistency checks).
    #[cfg(test)]
    pub(crate) fn entries(&self) -> Vec<(u64, TupleId)> {
        let mut entries: Vec<(u64, TupleId)> =
            self.single
                .iter()
                .map(|(&digest, &packed)| (digest, unpack(packed)))
                .chain(self.multi.iter().flat_map(|(&digest, ids)| {
                    ids.iter().map(move |&packed| (digest, unpack(packed)))
                }))
                .collect();
        entries.sort_by_key(|&(digest, id)| (digest, id.page_id, id.slot));
        entries
    }
}

/// The implicit-key index of one heap file.
pub(crate) enum KeyIndexState {
    /// Not built yet, or invalidated: the next lookup builds it with one
    /// heap scan.
    Unbuilt,
    /// Built and maintained on every slot insert and removal.
    Built(KeyIndex),
    /// The relation holds more versions than the index capacity: lookups
    /// scan the heap. GC resets this to `Unbuilt` so a shrunken relation
    /// is indexed again.
    Overflow,
}

impl<D: BlockDevice> HeapFile<D> {
    /// Limits the implicit-key index to `capacity` versions and discards
    /// the current index (the next lookup rebuilds it under the new bound).
    /// Test-only: production heaps use [`DEFAULT_KEY_INDEX_CAPACITY`].
    #[cfg(test)]
    pub(crate) fn set_key_index_capacity(&mut self, capacity: usize) {
        self.key_index_capacity = capacity;
        self.key_index = KeyIndexState::Unbuilt;
    }

    /// Discards the implicit-key index; the next lookup rebuilds it from
    /// the heap. Called whenever the heap changed in a way the index did
    /// not follow (a non-versioned write, or a write that failed part-way).
    pub(crate) fn invalidate_key_index(&mut self) {
        self.key_index = KeyIndexState::Unbuilt;
    }

    /// Records a new version slot in the index (when built).
    pub(super) fn key_index_note_insert(&mut self, tuple: &Tuple, tuple_id: TupleId) {
        if let KeyIndexState::Built(index) = &mut self.key_index {
            let digest = index.digest(tuple);
            if !index.insert(digest, tuple_id) || index.len() > self.key_index_capacity {
                self.key_index = KeyIndexState::Overflow;
            }
        }
    }

    /// Removes a reclaimed version slot from the index (when built).
    ///
    /// `tuple_bytes` are the slot's serialized tuple, read before the slot
    /// is freed. If they do not decode, the index can no longer locate the
    /// entry and is discarded instead.
    pub(super) fn key_index_note_remove(&mut self, tuple_bytes: &[u8], tuple_id: TupleId) {
        let KeyIndexState::Built(index) = &mut self.key_index else {
            return;
        };
        match super::deserialize_bounded::<Tuple>(tuple_bytes) {
            Ok(tuple) => {
                let digest = index.digest(&tuple);
                let removed = index.remove(digest, tuple_id);
                debug_assert!(removed, "implicit-key index missed a live version slot");
                if !removed {
                    self.key_index = KeyIndexState::Unbuilt;
                }
            }
            Err(_) => self.key_index = KeyIndexState::Unbuilt,
        }
    }

    /// Builds the index with one scan of the heap. Ends in `Overflow` when
    /// the relation holds more versions than the capacity.
    fn build_key_index(&mut self, version_pool: &mut VersionPool) -> Result<(), HeapError> {
        let mut index = KeyIndex::new();
        let mut overflow = false;
        let capacity = self.key_index_capacity;
        self.for_each_live_version(version_pool, &mut |tuple_id, _, tuple| {
            if overflow {
                return;
            }
            let digest = index.digest(&tuple);
            if !index.insert(digest, tuple_id) || index.len() > capacity {
                overflow = true;
            }
        })?;
        self.key_index = if overflow {
            KeyIndexState::Overflow
        } else {
            KeyIndexState::Built(index)
        };
        Ok(())
    }

    /// Visits every live version slot of the heap (any `xmin`/`xmax`) with
    /// its address, version metadata and tuple value.
    fn for_each_live_version(
        &mut self,
        version_pool: &mut VersionPool,
        visit: &mut dyn FnMut(TupleId, VersionMetadata, Tuple),
    ) -> Result<(), HeapError> {
        let mut buffer = version_pool.acquire_buffer()?;
        let mut page_id = 0;
        loop {
            let page = self.load_page(page_id)?;
            if page.is_empty() {
                break;
            }
            if is_versioned_page(page.data()) {
                decode_versioned_page_into(page.data(), &mut buffer.page.slots)?;
                for (slot, entry) in buffer.page.slots.iter().enumerate() {
                    let Some(entry) = entry else {
                        continue;
                    };
                    let bytes = slot_bytes(page.data(), entry.offset, entry.length)?;
                    let tuple: Tuple = super::deserialize_bounded(bytes)?;
                    let metadata = VersionMetadata {
                        xmin: entry.xmin,
                        xmax: entry.xmax,
                    };
                    // Slot numbers come from a `u32` slot count.
                    visit(
                        TupleId {
                            page_id,
                            slot: slot as u32,
                        },
                        metadata,
                        tuple,
                    );
                }
            }
            page_id += 1;
        }
        Ok(())
    }

    /// Returns the version metadata of every live version slot holding a
    /// tuple equal to `tuple` (any creator, any deleter).
    ///
    /// Uses the implicit-key index, building it on first use; when the
    /// relation overflows the index capacity, scans the heap instead.
    ///
    /// # Errors
    ///
    /// A device, page-decode or tuple-decode error, or
    /// [`HeapError::Mvcc`] when no page working buffer is free.
    pub(crate) fn versions_of(
        &mut self,
        tuple: &Tuple,
        version_pool: &mut VersionPool,
    ) -> Result<Vec<VersionMetadata>, HeapError> {
        if matches!(self.key_index, KeyIndexState::Unbuilt) {
            self.build_key_index(version_pool)?;
        }
        let mut versions = Vec::new();
        let lookup = match &self.key_index {
            KeyIndexState::Built(index) => {
                let digest = index.digest(tuple);
                let mut candidates = Vec::new();
                index.candidates(digest, &mut candidates);
                Some((digest, candidates))
            }
            KeyIndexState::Overflow | KeyIndexState::Unbuilt => None,
        };
        let Some((digest, candidates)) = lookup else {
            // Over capacity: scan the heap.
            self.for_each_live_version(version_pool, &mut |_, metadata, candidate| {
                if candidate == *tuple {
                    versions.push(metadata);
                }
            })?;
            return Ok(versions);
        };
        if candidates.is_empty() {
            return Ok(versions);
        }

        let mut buffer = version_pool.acquire_buffer()?;
        for tuple_id in candidates {
            let page = self.load_page(tuple_id.page_id)?;
            let entry = if page.is_empty() || !is_versioned_page(page.data()) {
                None
            } else {
                decode_versioned_page_into(page.data(), &mut buffer.page.slots)?;
                buffer
                    .page
                    .slots
                    .get(tuple_id.slot as usize)
                    .and_then(Option::as_ref)
            };
            // A freed slot is a stale address: the index must follow every
            // slot removal. (Harmless in release builds: it is skipped.)
            debug_assert!(entry.is_some(), "implicit-key index holds a freed slot");
            let Some(entry) = entry else {
                continue;
            };
            let bytes = slot_bytes(page.data(), entry.offset, entry.length)?;
            let candidate: Tuple = super::deserialize_bounded(bytes)?;
            if candidate == *tuple {
                versions.push(VersionMetadata {
                    xmin: entry.xmin,
                    xmax: entry.xmax,
                });
            } else if let KeyIndexState::Built(index) = &self.key_index {
                // Same digest, different tuple: a collision, or a slot that
                // was reused without the index noticing.
                debug_assert_eq!(
                    index.digest(&candidate),
                    digest,
                    "implicit-key index holds a reused slot"
                );
            }
        }
        Ok(versions)
    }

    /// Test-only consistency check: when the index is built, it must hold
    /// exactly the live version slots of the heap, each under its tuple's
    /// digest (compared with a full scan).
    #[cfg(test)]
    pub(crate) fn check_key_index(&mut self, version_pool: &mut VersionPool) -> Result<(), String> {
        let KeyIndexState::Built(index) = &self.key_index else {
            return Ok(());
        };
        let actual = index.entries();
        let seed = index.seed.clone();
        let mut expected = Vec::new();
        self.for_each_live_version(version_pool, &mut |tuple_id, _, tuple| {
            expected.push((seed.hash_one(&tuple), tuple_id));
        })
        .map_err(|e| format!("key index check scan failed: {e}"))?;
        expected.sort_by_key(|&(digest, id)| (digest, id.page_id, id.slot));
        if actual == expected {
            return Ok(());
        }
        let missing: Vec<_> = expected.iter().filter(|e| !actual.contains(e)).collect();
        let extra: Vec<_> = actual.iter().filter(|e| !expected.contains(e)).collect();
        Err(format!(
            "implicit-key index is stale: {} indexed slots, {} live slots on the heap; \
             missing from the index: {:?}; indexed but not on the heap: {:?}",
            actual.len(),
            expected.len(),
            missing.iter().take(8).map(|(_, id)| id).collect::<Vec<_>>(),
            extra.iter().take(8).map(|(_, id)| id).collect::<Vec<_>>(),
        ))
    }

    /// Whether the implicit-key index is currently built (tests).
    #[cfg(test)]
    pub(crate) fn key_index_is_built(&self) -> bool {
        matches!(self.key_index, KeyIndexState::Built(_))
    }

    /// Whether the relation overflowed the index capacity (tests).
    #[cfg(test)]
    pub(crate) fn key_index_overflowed(&self) -> bool {
        matches!(self.key_index, KeyIndexState::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(page_id: u64, slot: u32) -> TupleId {
        TupleId { page_id, slot }
    }

    #[test]
    fn test_pack_round_trips_and_rejects_out_of_range() {
        let tuple_id = id(123_456, 4095);
        assert_eq!(pack(tuple_id).map(unpack), Some(tuple_id));
        assert_eq!(pack(id(0, 1 << 16)), None);
        assert_eq!(pack(id(1 << 48, 0)), None);
    }

    #[test]
    fn test_insert_and_remove_track_single_and_multi_entries() {
        let mut index = KeyIndex::new();
        assert!(index.insert(7, id(0, 0)));
        assert!(index.insert(7, id(0, 0))); // idempotent
        assert_eq!(index.len(), 1);
        assert!(index.insert(7, id(1, 2)));
        assert!(index.insert(9, id(0, 1)));
        assert_eq!(index.len(), 3);

        let mut out = Vec::new();
        index.candidates(7, &mut out);
        out.sort_by_key(|t| (t.page_id, t.slot));
        assert_eq!(out, vec![id(0, 0), id(1, 2)]);

        assert!(index.remove(7, id(0, 0)));
        assert!(!index.remove(7, id(0, 0)));
        out.clear();
        index.candidates(7, &mut out);
        assert_eq!(out, vec![id(1, 2)]);
        assert!(index.remove(7, id(1, 2)));
        assert!(index.remove(9, id(0, 1)));
        assert_eq!(index.len(), 0);
        assert!(index.entries().is_empty());
    }

    #[test]
    fn test_equal_tuples_have_equal_digests() {
        use relvar_core::tuple;
        let index = KeyIndex::new();
        let a = tuple! { id: 1i64, name: "Alice" };
        let b = tuple! { name: "Alice", id: 1i64 };
        assert_eq!(index.digest(&a), index.digest(&b));
    }
}
