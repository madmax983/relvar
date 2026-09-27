//! # Bounded MVCC version pools
//!
//! This module implements the **operational** half of bounded MVCC: a
//! caller-owned, fixed-capacity pool of uncommitted-version records plus a
//! small pool of reusable page working buffers.
//!
//! ## Why two pools
//!
//! * **Version records** ([`VersionPool::claim`]) bound how many tuple
//!   versions may be uncommitted at once. Every versioned heap write
//!   (insert, update, delete) claims one record *before* touching any page;
//!   exhaustion returns the typed [`MvccError::VersionPoolExhausted`] and
//!   never panics. Records are released when their transaction commits or
//!   aborts ([`VersionPool::release_for_txn`]), or rolled back when the
//!   write that claimed them fails ([`VersionPool::release`]).
//! * **Page working buffers** ([`VersionPool::acquire_buffer`]) are the
//!   scratch space versioned page reads/writes use instead of allocating:
//!   a reusable slot-directory vector, an encoded page image, and a fixed
//!   slot-directory serialization scratch area. Snapshot reads iterate
//!   these buffers without allocating per version.
//!
//! ## Capacity model
//!
//! All backing storage is allocated once at construction:
//!
//! * `records`: `Vec<VersionRecord>` of `version_capacity`, with a free-index
//!   stack. `claim` pops an index; `release` / `release_for_txn` push freed
//!   indices back. No allocation after construction.
//! * `txn_claims`: a fixed-size table (default
//!   [`DEFAULT_MAX_CLAIMANT_TXNS`]) of per-transaction claim lists. Each
//!   record links into its transaction's list intrusively, so
//!   `release_for_txn` walks only that transaction's claims — O(versions),
//!   never O(pool capacity).
//! * `buffers`: `Vec<PageWorkingSet>` of `buffer_capacity` (default
//!   [`DEFAULT_VERSION_BUFFER_CAPACITY`]). Each working set pre-reserves a
//!   full page image and a fixed `[u8; SLOT_DIR_SCRATCH_SIZE]` scratch
//!   array. Page work checks a buffer out through a guard
//!   ([`VersionBufferGuard`]) that returns it on drop, so early returns
//!   cannot leak buffers.
//!
//! Page work nests at most one buffer deep (an operation holds a single
//! buffer while decoding, mutating, and re-encoding one page at a time),
//! so the default buffer capacity is generous; exhaustion is still a typed
//! error ([`MvccError::VersionBuffersExhausted`]), never a panic.
//!
//! ## Handles and stale-handle rejection
//!
//! `claim` returns an opaque generational [`VersionHandle`]: the record
//! index plus a generation counter bumped on every claim. `release`
//! verifies the generation (and liveness) before freeing, so a stale
//! handle — from an already-released claim, or a record re-claimed by a
//! later transaction — releases nothing instead of corrupting the pool.
//! The handle is `#[must_use]`: dropping a claim without releasing it
//! leaks the record until its transaction ends.
//!
//! ## Settled design questions
//!
//! * **Chain ownership** lives on pages, not in the pool: each slot entry
//!   carries `xmin`/`xmax` and the `prev_version` chain link. The pool is a
//!   capacity accountant, not a metadata store, so there is nothing else
//!   to go stale.
//! * **Tuple-byte capacity**: tuple bytes exist in exactly two places, both
//!   bounded. At rest they live on heap pages: each page holds at most
//!   [`USABLE_PAGE_SIZE_V2`] (4088) usable bytes, enforced per tuple by the
//!   heap's size check
//!   ([`HeapError::TupleTooLarge`](crate::storage::heap::HeapError::TupleTooLarge))
//!   and per page by
//!   [`HeapError::PageFull`](crate::storage::heap::HeapError::PageFull).
//!   In flight they live only in the pool's page working buffers:
//!   `buffer_capacity` buffers, each pre-reserving one full page image
//!   ([`USABLE_PAGE_SIZE_V2`] bytes) plus one fixed-size serialization
//!   scratch array ([`SLOT_DIR_SCRATCH_SIZE`] bytes) — all allocated once at construction and never grown (pinned by the
//!   `page_working_set_byte_capacities_are_fixed` test). Corrupt pages
//!   claiming more slot-directory entries than the structural bound
//!   ([`MAX_SLOT_DIR_ENTRIES`](relvar_storage_core::slotted::MAX_SLOT_DIR_ENTRIES))
//!   are rejected by the decoder, so hostile input cannot grow the slot
//!   vector either. [`VersionRecord`]s hold no tuple bytes at all —
//!   transaction/page/slot identifiers, generation, and intrusive list
//!   links only. The pool's tuple-byte capacity is therefore
//!   `buffer_capacity × (4088 + 8192)` bytes, fixed at construction: not a
//!   second independent budget, but the page-size bound materialized as
//!   reusable scratch, one page at a time.
//! * **GC reuse**: records are recycled on commit/abort; freed page slots
//!   are reused for new versions. Reclamation stays snapshot-constrained:
//!   GC only removes versions dead to every live snapshot (it consults the
//!   transaction pool's horizon and the committed set).
//! * **Recovery**: the pool is volatile operational state. It is rebuilt
//!   empty on open, so no pool records are owed to pre-crash transactions
//!   and new claims start fresh. Recovery replays the WAL to rebuild the
//!   committed set; uncommitted inserts whose records never became
//!   durable are simply never replayed. Note the asymmetry: a heap page
//!   write whose WAL Insert record was lost to the log buffer can remain
//!   physically present on disk, but it stays invisible forever — its
//!   transaction ID is never reused (Begin records are flushed before any
//!   write) and never joins the committed set, so no snapshot can observe
//!   it and GC reclaims it. Recovery does not need to physically undo
//!   what it never saw.
//! * **Atomicity**: the claim happens before any page mutation, and every
//!   validation the heap can perform without mutating (tuple size limits,
//!   tuple existence, page decoding, buffer checkout) happens before the
//!   first mutation too. A write that fails after claiming releases its
//!   handle, so validation and exhaustion failures never leave partial
//!   state. Single-phase writes (insert, delete) are atomic: a failure
//!   before the single page store mutates nothing. Update is two-phase
//!   (mark the old version, insert the replacement): if the replacement
//!   insert fails, the heap rolls the marking back; if that rollback
//!   fails too, the heap returns
//!   [`HeapError::UpdateRollbackFailed`](crate::storage::heap::HeapError::UpdateRollbackFailed)
//!   instead of discarding the error. That error poisons the transaction
//!   — the old version stays marked deleted by it, so the caller must
//!   abort; committing would hide the old version without its
//!   replacement and silently lose the tuple. Aborting the transaction
//!   (via the WAL) is the engine's job; the pool only guarantees its
//!   claims are released.
//!
//! ## Ownership
//!
//! The pool is caller-owned: [`PersistentEngine`](crate::PersistentEngine)
//! owns one and threads `&mut` access into the heap layer. Long-lived
//! snapshots constrain *version reclamation* (GC consults the transaction
//! pool's horizon), but the version pool itself only tracks uncommitted
//! versions — committed versions live on pages and are reclaimed by GC.

use super::pool::{DEFAULT_TXN_POOL_CAPACITY, MvccError};
use crate::wal::TransactionId;
use relvar_storage_core::slotted::{
    MAX_SLOT_DIR_ENTRIES, SLOT_DIR_SCRATCH_SIZE, USABLE_PAGE_SIZE_V2, VERSIONED_PAGE_MAGIC,
    VersionedSlottedPage,
};
use std::ops::{Deref, DerefMut};

/// Default bound on simultaneously uncommitted tuple versions.
///
/// Sized comfortably above the largest single-transaction write observed in
/// the repository (a 1,000-tuple `store_relation`): one record is a few
/// dozen bytes, so the default pool is a small fixed cost. Callers with
/// bigger transactions raise it via [`VersionPool::new`].
pub const DEFAULT_VERSION_POOL_CAPACITY: usize = 65_536;

/// Default number of reusable page working buffers.
///
/// Versioned page work holds at most one buffer at a time, so four is
/// ample headroom; exhaustion stays a typed error.
pub const DEFAULT_VERSION_BUFFER_CAPACITY: usize = 4;

/// Default bound on distinct transactions holding version claims at once.
///
/// Matches [`DEFAULT_TXN_POOL_CAPACITY`]: outstanding claims always belong
/// to live transactions, so the engine wires its transaction-pool capacity
/// here and the bound can never trigger spuriously.
pub const DEFAULT_MAX_CLAIMANT_TXNS: usize = DEFAULT_TXN_POOL_CAPACITY;

/// Opaque handle to one claimed version record.
///
/// Returned by [`VersionPool::claim`]; [`VersionPool::release`] consumes
/// it to recycle the record. The generation rejects stale handles: a
/// handle from an earlier claim generation never releases a record that
/// was re-claimed later (see the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a claimed version record stays claimed until its handle is released"]
pub struct VersionHandle {
    /// Index into the pool's record array.
    index: usize,
    /// Claim generation of the record when the handle was issued.
    generation: u32,
}

/// One uncommitted tuple version's pool record.
///
/// The record exists to *bound* uncommitted versions, not to duplicate the
/// version metadata already stored on the page (`xmin`/`xmax`/chain link).
/// Records link into their transaction's claim list intrusively via
/// `next_for_txn`, so [`VersionPool::release_for_txn`] walks only that
/// transaction's records.
#[derive(Debug, Clone, Copy)]
struct VersionRecord {
    /// Transaction that owns the uncommitted version.
    txn_id: TransactionId,
    /// Claim generation; bumped on every claim for stale-handle rejection.
    generation: u32,
    /// Whether this slot currently holds a claimed record.
    live: bool,
    /// Next record in the owning transaction's claim list.
    next_for_txn: Option<usize>,
}

/// One transaction's claim list: the head of its intrusive record chain.
#[derive(Debug, Clone, Copy)]
struct TxnClaimSlot {
    txn_id: TransactionId,
    head: Option<usize>,
}

/// Reusable scratch space for one versioned page read-modify-write cycle.
///
/// All members are allocated once (at pool construction) and reused across
/// operations, so versioned page work performs no allocation after the
/// pool is built:
///
/// * `page`: the decoded slot directory; its `slots` vector keeps its
///   capacity across pages.
/// * `page_image`: the encoded page image being built; pre-reserved for a
///   full [`USABLE_PAGE_SIZE_V2`] page.
/// * `slot_scratch`: fixed-size scratch for serializing the slot directory.
#[derive(Debug)]
pub struct PageWorkingSet {
    /// Decoded slot directory of the page under work (slots vector reused).
    pub page: VersionedSlottedPage,
    /// Encoded page image under construction (capacity pre-reserved).
    pub page_image: Vec<u8>,
    /// Fixed scratch for slot-directory serialization (never reallocates).
    pub slot_scratch: [u8; SLOT_DIR_SCRATCH_SIZE],
}

impl PageWorkingSet {
    fn new() -> Self {
        // Pre-reserve the structural entry bound (see
        // `relvar_storage_core::slotted::MAX_SLOT_DIR_ENTRIES`): decoding an
        // honest page's slot directory never grows this vector, and the
        // decoder rejects corrupt pages claiming more entries.
        let slots = Vec::with_capacity(MAX_SLOT_DIR_ENTRIES);
        Self {
            page: VersionedSlottedPage {
                magic: VERSIONED_PAGE_MAGIC,
                slot_count: 0,
                slots,
            },
            page_image: Vec::with_capacity(USABLE_PAGE_SIZE_V2),
            slot_scratch: [0u8; SLOT_DIR_SCRATCH_SIZE],
        }
    }

    /// Resets the working set for a fresh page; keeps all capacities.
    pub fn reset(&mut self) {
        self.page.magic = VERSIONED_PAGE_MAGIC;
        self.page.slot_count = 0;
        self.page.slots.clear();
        self.page_image.clear();
    }
}

/// Bounded, caller-owned pool of uncommitted-version records and page
/// working buffers.
///
/// See the [module](self) documentation for the capacity model.
#[derive(Debug)]
pub struct VersionPool {
    /// Pre-allocated version records; a slot is live when not on the free list.
    records: Vec<VersionRecord>,
    /// Indices of free record slots.
    free_records: Vec<usize>,
    /// Fixed-size table of per-transaction claim lists.
    txn_claims: Vec<Option<TxnClaimSlot>>,
    /// Pre-allocated page working buffers.
    buffers: Vec<PageWorkingSet>,
    /// Indices of checked-in buffers.
    free_buffers: Vec<usize>,
}

impl VersionPool {
    /// Creates a pool with `version_capacity` version records, the default
    /// number of page working buffers, and the default claimant bound.
    ///
    /// A zero `version_capacity` is clamped to one so the pool can always
    /// make progress on a single-version transaction.
    pub fn new(version_capacity: usize) -> Self {
        Self::with_capacities(
            version_capacity,
            DEFAULT_VERSION_BUFFER_CAPACITY,
            DEFAULT_MAX_CLAIMANT_TXNS,
        )
    }

    /// Creates a pool with explicit record and buffer capacities and the
    /// default claimant bound.
    ///
    /// Capacities are fixed at construction; all backing storage is
    /// allocated here, never on the write path.
    pub fn with_buffer_capacity(version_capacity: usize, buffer_capacity: usize) -> Self {
        Self::with_capacities(version_capacity, buffer_capacity, DEFAULT_MAX_CLAIMANT_TXNS)
    }

    /// Creates a pool with explicit record, buffer, and claimant capacities.
    ///
    /// `max_claimant_txns` bounds how many distinct transactions may hold
    /// version claims at once; the engine passes its transaction-pool
    /// capacity here. All backing storage is allocated here, never on the
    /// write path.
    pub fn with_capacities(
        version_capacity: usize,
        buffer_capacity: usize,
        max_claimant_txns: usize,
    ) -> Self {
        let version_capacity = version_capacity.max(1);
        let buffer_capacity = buffer_capacity.max(1);
        let max_claimant_txns = max_claimant_txns.max(1);

        let mut free_records = Vec::with_capacity(version_capacity);
        for index in (0..version_capacity).rev() {
            free_records.push(index);
        }
        let mut free_buffers = Vec::with_capacity(buffer_capacity);
        for index in (0..buffer_capacity).rev() {
            free_buffers.push(index);
        }

        Self {
            records: (0..version_capacity)
                .map(|_| VersionRecord {
                    txn_id: TransactionId::new(0),
                    generation: 0,
                    live: false,
                    next_for_txn: None,
                })
                .collect(),
            free_records,
            txn_claims: (0..max_claimant_txns).map(|_| None).collect(),
            buffers: (0..buffer_capacity)
                .map(|_| PageWorkingSet::new())
                .collect(),
            free_buffers,
        }
    }

    /// Number of version records the pool was built with.
    pub fn version_capacity(&self) -> usize {
        self.records.len()
    }

    /// Number of version records currently claimed (uncommitted versions).
    pub fn used_versions(&self) -> usize {
        self.records.len() - self.free_records.len()
    }

    /// Bound on distinct transactions holding version claims at once.
    pub fn max_claimant_txns(&self) -> usize {
        self.txn_claims.len()
    }

    /// Number of page working buffers the pool was built with.
    pub fn buffer_capacity(&self) -> usize {
        self.buffers.len()
    }

    /// Number of page working buffers currently checked out.
    pub fn used_buffers(&self) -> usize {
        self.buffers.len() - self.free_buffers.len()
    }

    /// Backing-store capacities `(records, free stack, claimant table)`.
    ///
    /// Test hook for the v0.9 contract: the write path must never grow the
    /// pool's backing storage after construction. Only available in test
    /// builds.
    #[cfg(test)]
    pub fn backing_capacities(&self) -> (usize, usize, usize) {
        (
            self.records.capacity(),
            self.free_records.capacity(),
            self.txn_claims.capacity(),
        )
    }

    /// Claims one version record for an uncommitted version written by
    /// `txn_id`, returning its handle.
    ///
    /// Must be called *before* the heap write it accounts for, so a
    /// capacity failure happens before any page is mutated and the write
    /// stays atomic. On exhaustion returns the typed
    /// [`MvccError::VersionPoolExhausted`] (or
    /// [`MvccError::VersionClaimantsExhausted`] when too many distinct
    /// transactions hold claims); never panics. Performs no allocation:
    /// the record comes from the free list and the transaction's claim
    /// list from the fixed table.
    pub fn claim(&mut self, txn_id: TransactionId) -> Result<VersionHandle, MvccError> {
        let index = self
            .free_records
            .pop()
            .ok_or_else(|| MvccError::VersionPoolExhausted {
                used: self.used_versions(),
                capacity: self.version_capacity(),
            })?;
        let slot = match self.claim_slot_for(txn_id) {
            Some(slot) => slot,
            None => {
                // The claim did not happen: hand the record back so the
                // failed claim leaves the pool exactly as it found it.
                self.free_records.push(index);
                return Err(MvccError::VersionClaimantsExhausted {
                    active: self.active_claimants(),
                    capacity: self.max_claimant_txns(),
                });
            }
        };

        let generation = self.records[index].generation.wrapping_add(1);
        let head = self.txn_claims[slot].and_then(|s| s.head);
        self.records[index] = VersionRecord {
            txn_id,
            generation,
            live: true,
            next_for_txn: head,
        };
        if let Some(slot) = self.txn_claims[slot].as_mut() {
            slot.head = Some(index);
        }
        Ok(VersionHandle { index, generation })
    }

    /// Releases the record `handle` claims.
    ///
    /// Stale handles are rejected without panicking: a handle for an
    /// already-released record, a re-claimed record (generation mismatch),
    /// or an out-of-range index releases nothing. Returns whether a live
    /// record was actually released.
    ///
    /// The node is unlinked from its transaction's claim list before the
    /// record returns to the free list. That unlink is load-bearing: the
    /// freed record can be re-claimed immediately — possibly by another
    /// transaction — and `claim` overwrites `next_for_txn`. Leaving the
    /// stale node linked would let the old list reach the recycled node
    /// and corrupt the list into a cycle.
    pub fn release(&mut self, handle: VersionHandle) -> bool {
        let index = handle.index;
        let owning_txn = match self.records.get(index) {
            Some(record) if record.live && record.generation == handle.generation => record.txn_id,
            _ => return false,
        };

        // Splice the node out of its transaction's claim list. The live
        // record is linked there exactly once: `claim` pushes it, and only
        // this function or `release_for_txn` removes it (the latter also
        // retires the slot, which would have made the record non-live).
        // A single pass over the slots finds the owning transaction and
        // unlinks the node, so no `expect` is needed to re-fetch the slot.
        for entry in self.txn_claims.iter_mut() {
            let Some(state) = entry.as_mut() else {
                continue;
            };
            if state.txn_id != owning_txn {
                continue;
            }
            let mut prev: Option<usize> = None;
            let mut current = state.head;
            while let Some(node) = current {
                if node == index {
                    let next = self.records[node].next_for_txn.take();
                    match prev {
                        Some(prev_node) => self.records[prev_node].next_for_txn = next,
                        None => state.head = next,
                    }
                    break;
                }
                prev = current;
                current = self.records[node].next_for_txn;
            }
            // Transaction IDs occupy at most one claim slot.
            break;
        }

        let record = &mut self.records[index];
        record.live = false;
        self.free_records.push(index);
        true
    }

    /// Releases every version record owned by `txn_id`.
    ///
    /// Called when the transaction commits or aborts; its versions are no
    /// longer uncommitted. Infallible and idempotent: releasing a
    /// transaction with no records returns `0`. Returns the number of
    /// records released. Runs in time proportional to the transaction's
    /// own claims — never in pool capacity — and frees the transaction's
    /// claim-list slot for reuse.
    pub fn release_for_txn(&mut self, txn_id: TransactionId) -> usize {
        let Some(slot) = self
            .txn_claims
            .iter()
            .position(|s| matches!(s, Some(s) if s.txn_id == txn_id))
        else {
            return 0;
        };
        let mut released = 0;
        let mut next = self.txn_claims[slot].and_then(|s| s.head);
        while let Some(index) = next {
            // Links are only ever written by `claim` with valid indices,
            // and `release` unlinks singly-released nodes, so every node
            // reached here is live and owned by this transaction. The
            // guard stays as defense-in-depth: a node that somehow is not
            // is skipped rather than freed twice.
            let record = &mut self.records[index];
            next = record.next_for_txn.take();
            if record.live && record.txn_id == txn_id {
                record.live = false;
                self.free_records.push(index);
                released += 1;
            }
        }
        self.txn_claims[slot] = None;
        released
    }

    /// Index of `txn_id`'s claim-list slot, filling an empty slot when the
    /// transaction claims for the first time. `None` when the fixed table
    /// is full.
    fn claim_slot_for(&mut self, txn_id: TransactionId) -> Option<usize> {
        if let Some(pos) = self
            .txn_claims
            .iter()
            .position(|s| matches!(s, Some(s) if s.txn_id == txn_id))
        {
            return Some(pos);
        }
        let pos = self.txn_claims.iter().position(|s| s.is_none())?;
        self.txn_claims[pos] = Some(TxnClaimSlot { txn_id, head: None });
        Some(pos)
    }

    /// Number of distinct transactions currently holding claims.
    fn active_claimants(&self) -> usize {
        self.txn_claims.iter().filter(|s| s.is_some()).count()
    }

    /// Checks out a page working buffer.
    ///
    /// The returned guard dereferences to the [`PageWorkingSet`] and
    /// returns the buffer to the pool on drop, so early returns cannot
    /// leak it. On exhaustion returns the typed
    /// [`MvccError::VersionBuffersExhausted`]; never panics.
    pub fn acquire_buffer(&mut self) -> Result<VersionBufferGuard<'_>, MvccError> {
        let index = self.try_acquire_buffer_index()?;
        Ok(VersionBufferGuard { pool: self, index })
    }

    /// Checks out a page working buffer, returning its raw index.
    ///
    /// Private: the public [`acquire_buffer`](Self::acquire_buffer) wraps
    /// this in an RAII guard. Tests use this directly to observe pool state
    /// between checkout and release, which the guard's borrow would forbid.
    fn try_acquire_buffer_index(&mut self) -> Result<usize, MvccError> {
        let index = self
            .free_buffers
            .pop()
            .ok_or(MvccError::VersionBuffersExhausted {
                used: self.used_buffers(),
                capacity: self.buffer_capacity(),
            })?;
        self.buffers[index].reset();
        Ok(index)
    }

    /// Returns the working set for a checked-out buffer index.
    ///
    /// Test-only: the public [`acquire_buffer`](Self::acquire_buffer) wraps
    /// the index in an RAII guard, whose borrow forbids observing pool
    /// state mid-checkout.
    #[cfg(test)]
    fn buffer_mut(&mut self, index: usize) -> &mut PageWorkingSet {
        &mut self.buffers[index]
    }

    /// Returns a checked-out buffer to the free list.
    fn release_buffer(&mut self, index: usize) {
        self.free_buffers.push(index);
    }
}

/// Guard for a checked-out page working buffer.
///
/// Dereferences to [`PageWorkingSet`]; dropping the guard returns the
/// buffer to the pool.
#[derive(Debug)]
pub struct VersionBufferGuard<'a> {
    pool: &'a mut VersionPool,
    index: usize,
}

impl Deref for VersionBufferGuard<'_> {
    type Target = PageWorkingSet;

    fn deref(&self) -> &Self::Target {
        &self.pool.buffers[self.index]
    }
}

impl DerefMut for VersionBufferGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pool.buffers[self.index]
    }
}

impl Drop for VersionBufferGuard<'_> {
    fn drop(&mut self) {
        self.pool.release_buffer(self.index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txn(id: u64) -> TransactionId {
        TransactionId::new(id)
    }

    #[test]
    fn claim_returns_handle_and_release_recycles() {
        let mut pool = VersionPool::new(4);
        assert_eq!(pool.used_versions(), 0);

        let h1 = pool.claim(txn(1)).unwrap();
        let h2 = pool.claim(txn(1)).unwrap();
        let _h3 = pool.claim(txn(2)).unwrap();
        assert_eq!(pool.used_versions(), 3);

        assert!(pool.release(h1));
        assert_eq!(pool.used_versions(), 2);
        // Releasing the same handle again releases nothing.
        assert!(!pool.release(h1));
        assert_eq!(pool.used_versions(), 2);

        assert!(pool.release(h2));
        assert_eq!(pool.used_versions(), 1);
    }

    #[test]
    fn release_rejects_stale_handle_after_reclaim() {
        let mut pool = VersionPool::new(1);

        let stale = pool.claim(txn(1)).unwrap();
        assert!(pool.release(stale));
        assert_eq!(pool.used_versions(), 0);

        // The freed record is re-claimed: same index, new generation.
        let fresh = pool.claim(txn(2)).unwrap();
        assert_eq!(pool.used_versions(), 1);

        // The stale handle must not release the new owner's record.
        assert!(!pool.release(stale));
        assert_eq!(pool.used_versions(), 1);

        // The fresh handle still works.
        assert!(pool.release(fresh));
        assert_eq!(pool.used_versions(), 0);
    }

    #[test]
    fn release_rejects_out_of_range_handle() {
        let mut pool = VersionPool::new(2);
        let bogus = VersionHandle {
            index: 999,
            generation: 1,
        };
        assert!(!pool.release(bogus));
        assert_eq!(pool.used_versions(), 0);
    }

    #[test]
    fn release_for_txn_is_per_transaction_and_idempotent() {
        let mut pool = VersionPool::new(4);

        let _a1 = pool.claim(txn(1)).unwrap();
        let _a2 = pool.claim(txn(1)).unwrap();
        let _b1 = pool.claim(txn(2)).unwrap();
        assert_eq!(pool.used_versions(), 3);

        // Releasing one transaction leaves the other's records claimed.
        assert_eq!(pool.release_for_txn(txn(1)), 2);
        assert_eq!(pool.used_versions(), 1);

        // Idempotent: releasing again frees nothing.
        assert_eq!(pool.release_for_txn(txn(1)), 0);
        assert_eq!(pool.release_for_txn(txn(7)), 0);
        assert_eq!(pool.release_for_txn(txn(2)), 1);
        assert_eq!(pool.used_versions(), 0);
    }

    #[test]
    fn single_release_then_bulk_release_does_not_double_free() {
        let mut pool = VersionPool::new(4);

        let h1 = pool.claim(txn(1)).unwrap();
        let _h2 = pool.claim(txn(1)).unwrap();
        assert!(pool.release(h1));

        // The singly-released node is still linked; the bulk release must
        // skip it rather than freeing the index twice.
        assert_eq!(pool.release_for_txn(txn(1)), 1);
        assert_eq!(pool.used_versions(), 0);

        // The free list is consistent: all four records claimable again.
        for _ in 0..4 {
            let _h = pool.claim(txn(9)).unwrap();
        }
        assert_eq!(pool.used_versions(), 4);
    }

    #[test]
    fn exhaustion_returns_typed_error_not_panic() {
        let mut pool = VersionPool::new(2);
        let _h1 = pool.claim(txn(1)).unwrap();
        let _h2 = pool.claim(txn(2)).unwrap();

        let err = pool.claim(txn(3)).unwrap_err();
        assert!(
            matches!(
                err,
                MvccError::VersionPoolExhausted {
                    used: 2,
                    capacity: 2
                }
            ),
            "expected typed VersionPoolExhausted, got {err:?}"
        );

        // The pool still works after exhaustion: release and reclaim.
        pool.release_for_txn(txn(1));
        let _h3 = pool.claim(txn(3)).unwrap();
        assert_eq!(pool.used_versions(), 2);
    }

    #[test]
    fn claimants_exhaustion_returns_typed_error_not_panic() {
        let mut pool = VersionPool::with_capacities(8, 1, 2);
        let _h1 = pool.claim(txn(1)).unwrap();
        let _h2 = pool.claim(txn(2)).unwrap();

        // Plenty of version records left, but the claimant table is full.
        let err = pool.claim(txn(3)).unwrap_err();
        assert!(
            matches!(
                err,
                MvccError::VersionClaimantsExhausted {
                    active: 2,
                    capacity: 2
                }
            ),
            "expected typed VersionClaimantsExhausted, got {err:?}"
        );

        // The failed claim left the pool exactly as it found it: the record
        // was handed back, so txn(3) can still be served after txn(1) ends.
        assert_eq!(pool.used_versions(), 2);
        pool.release_for_txn(txn(1));
        let h3 = pool.claim(txn(3)).unwrap();
        assert!(pool.release(h3));
        assert_eq!(pool.used_versions(), 1);
    }

    #[test]
    fn claim_performs_no_allocation_after_construction() {
        let mut pool = VersionPool::with_capacities(64, 2, 8);
        let records_cap = pool.records.capacity();
        let free_cap = pool.free_records.capacity();
        let claims_cap = pool.txn_claims.capacity();

        // Claim across several transactions, release singly and in bulk:
        // none of it may grow the backing storage.
        let mut handles = Vec::new();
        for i in 0..32 {
            handles.push(pool.claim(txn(i % 4 + 1)).unwrap());
        }
        for h in handles.drain(..) {
            assert!(pool.release(h));
        }
        for i in 0..16 {
            let _h = pool.claim(txn(i % 4 + 1)).unwrap();
        }
        pool.release_for_txn(txn(2));

        assert_eq!(pool.records.capacity(), records_cap);
        assert_eq!(pool.free_records.capacity(), free_cap);
        assert_eq!(pool.txn_claims.capacity(), claims_cap);
    }

    #[test]
    fn single_release_unlinks_so_recycled_index_cannot_cycle_claim_list() {
        // Two records: the freed index is definitely recycled below.
        let mut pool = VersionPool::with_capacities(2, 1, 4);

        let a = pool.claim(txn(1)).unwrap();
        let _b = pool.claim(txn(1)).unwrap();
        // Singly release `a`; its index returns to the free list while `b`
        // stays live in txn(1)'s claim list.
        assert!(pool.release(a));
        assert_eq!(pool.used_versions(), 1);

        // Another transaction claims and must recycle `a`'s index (the
        // only free record). If `release` left the stale node linked,
        // the recycled `next_for_txn` would corrupt txn(1)'s list into a
        // cycle and the bulk release below would never terminate.
        let c = pool.claim(txn(2)).unwrap();
        assert_eq!(c.index, a.index);
        assert_ne!(c.generation, a.generation);

        // Both bulk releases terminate and free exactly the live records.
        assert_eq!(pool.release_for_txn(txn(1)), 1);
        assert_eq!(pool.release_for_txn(txn(2)), 1);
        assert_eq!(pool.used_versions(), 0);

        // The pool is fully reusable afterwards.
        let d = pool.claim(txn(3)).unwrap();
        assert!(pool.release(d));
    }

    #[test]
    fn zero_capacity_clamps_to_one() {
        let mut pool = VersionPool::new(0);
        assert_eq!(pool.version_capacity(), 1);
        let _h = pool.claim(txn(1)).unwrap();
        assert!(pool.claim(txn(2)).is_err());
    }

    #[test]
    fn buffer_acquire_release_reuses_working_set() {
        let mut pool = VersionPool::with_buffer_capacity(8, 1);
        assert_eq!(pool.used_buffers(), 0);

        // The raw index API lets the test observe pool state mid-checkout
        // (the RAII guard's borrow would forbid this).
        let index = pool.try_acquire_buffer_index().unwrap();
        assert_eq!(pool.used_buffers(), 1);
        // Second checkout fails typed while the only buffer is out.
        assert!(matches!(
            pool.try_acquire_buffer_index().unwrap_err(),
            MvccError::VersionBuffersExhausted { .. }
        ));
        pool.buffer_mut(index).page.slots.push(None);
        pool.buffer_mut(index)
            .page_image
            .extend_from_slice(&[1, 2, 3]);
        let first_ptr = pool.buffer_mut(index).page.slots.as_ptr();
        pool.release_buffer(index);
        assert_eq!(pool.used_buffers(), 0);

        // Re-acquiring yields the same backing storage (no reallocation),
        // reset for the next page.
        let index = pool.try_acquire_buffer_index().unwrap();
        assert!(pool.buffer_mut(index).page.slots.is_empty());
        assert!(pool.buffer_mut(index).page_image.is_empty());
        assert_eq!(pool.buffer_mut(index).page.slots.as_ptr(), first_ptr);
        assert!(pool.buffer_mut(index).page_image.capacity() >= USABLE_PAGE_SIZE_V2);
        assert_eq!(
            pool.buffer_mut(index).slot_scratch.len(),
            SLOT_DIR_SCRATCH_SIZE
        );
        pool.release_buffer(index);
        assert_eq!(pool.used_buffers(), 0);
    }

    /// Pins the pool's tuple-byte capacity: every page working buffer
    /// pre-reserves enough storage for one full page image, one full honest
    /// slot directory, and the serialization scratch — and using the buffer
    /// never grows any of it. Together with the decoder's entry bound
    /// (`MAX_SLOT_DIR_ENTRIES`), this is the mechanical proof that the
    /// pool's tuple-byte capacity is fixed at construction.
    #[test]
    fn page_working_set_byte_capacities_are_fixed() {
        let mut pool = VersionPool::with_capacities(8, 3, 8);
        for index in 0..pool.buffer_capacity() {
            let buffer = pool.buffer_mut(index);
            // Tuple bytes: at least one full usable page image, so page
            // work never reallocates the image.
            assert!(buffer.page_image.capacity() >= USABLE_PAGE_SIZE_V2);
            // Slot metadata: at least the structural entry bound, so
            // decoding any honest page never grows the slot vector.
            assert!(buffer.page.slots.capacity() >= MAX_SLOT_DIR_ENTRIES);
            // Serialization scratch: a fixed-size array; it cannot grow.
            assert_eq!(buffer.slot_scratch.len(), SLOT_DIR_SCRATCH_SIZE);

            let image_cap = buffer.page_image.capacity();
            let slots_cap = buffer.page.slots.capacity();
            // Working the buffer (reset + simulated decode fill) must not
            // grow the backing storage.
            buffer.reset();
            buffer.page.slots.resize(MAX_SLOT_DIR_ENTRIES, None);
            buffer.page_image.resize(USABLE_PAGE_SIZE_V2, 0);
            buffer.reset();
            assert_eq!(buffer.page_image.capacity(), image_cap);
            assert_eq!(buffer.page.slots.capacity(), slots_cap);
        }
    }

    #[test]
    fn buffer_guard_returns_buffer_on_drop() {
        let mut pool = VersionPool::with_buffer_capacity(8, 1);
        {
            let mut guard = pool.acquire_buffer().unwrap();
            guard.page.slots.push(None);
            assert_eq!(guard.page.slots.len(), 1);
            // The pool is mutably borrowed by the guard here; its state is
            // observable again once the guard drops.
        }
        assert_eq!(pool.used_buffers(), 0);
        // The buffer came back and was reset.
        {
            let guard = pool.acquire_buffer().unwrap();
            assert!(guard.page.slots.is_empty());
        }
        assert_eq!(pool.used_buffers(), 0);
    }

    #[test]
    fn buffer_guard_returns_on_early_return() {
        fn fallible(pool: &mut VersionPool) -> Result<(), MvccError> {
            let _guard = pool.acquire_buffer()?;
            Err(MvccError::VersionPoolExhausted {
                used: 0,
                capacity: 0,
            })
        }

        let mut pool = VersionPool::with_buffer_capacity(8, 1);
        assert!(fallible(&mut pool).is_err());
        // The guard's Drop returned the buffer despite the early return.
        assert_eq!(pool.used_buffers(), 0);
        pool.acquire_buffer().unwrap();
    }
}
