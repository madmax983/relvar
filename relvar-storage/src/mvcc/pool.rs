//! # Bounded MVCC transaction pool
//!
//! [`TxnPool`] is the bounded, caller-owned home for MVCC transaction state.
//! It replaces the old unbounded `HashMap<TransactionId, TransactionSnapshot>`
//! active-transaction table: every slot, every free-list entry, and every
//! history record is pre-allocated at construction, so [`TxnPool::begin`],
//! [`TxnPool::commit`], and [`TxnPool::abort`] never allocate.
//!
//! ## Why a pool, and why a history ring
//!
//! Snapshot Isolation needs to answer one question per tuple version: "was the
//! transaction that created or deleted this version still running when my
//! snapshot was taken?" The old design answered it by copying the whole active
//! set into every snapshot (a `HashSet` per snapshot, cloned into the table) —
//! allocation on every begin, on every read-committed read, on every
//! repeatable-read snapshot fetch.
//!
//! The pool answers it without copying anything: a snapshot is just
//! `{txn_id, snapshot_lsn, horizon}` (`Copy`, stack-only). Liveness is derived
//! from two bounded structures:
//!
//! - **Live slots**: one per concurrent transaction. A transaction ID found in
//!   a live slot is uncommitted, hence active at any snapshot taken while it
//!   lives.
//! - **History ring**: finished transactions with their begin/commit LSNs.
//!   Transaction T was active at snapshot S (taken at LSN `s`) iff
//!   `begin_lsn(T) <= s` and T had not committed by `s`
//!   (`commit_lsn(T) > s`). Aborted transactions report "not active": their
//!   versions are invisible through the committed set anyway, so every
//!   visibility path stays correct (see [`TxnPool::is_active_at`]).
//!
//! History records are evicted oldest-first once they are provably invisible
//! to every live snapshot — committed at or before the oldest live snapshot's
//! LSN; aborted records are always evictable. If the ring fills with no
//! evictable record (a long-lived snapshot under heavy churn), `commit` and
//! `abort` return [`MvccError::HistoryExhausted`] instead of growing.
//!
//! ## Capacity policy
//!
//! The pool is constructed with a fixed capacity: the maximum number of
//! concurrently live transactions. [`TxnPool::begin`] on a full pool returns
//! [`MvccError::PoolExhausted`] — a typed error, never a panic. Pick the
//! capacity for the deployment: [`DEFAULT_TXN_POOL_CAPACITY`] (64) suits the
//! host engine; embedded callers pass their own bound.
//!
//! ## TTM Compliance
//!
//! This module is entirely internal to the storage layer (Physical Data
//! Independence). The logical layer never sees transaction IDs, slots, or
//! history records.

use crate::wal::{Lsn, TransactionId};
use std::collections::VecDeque;
use thiserror::Error;

/// Default bound on concurrently live transactions for the host engine.
pub const DEFAULT_TXN_POOL_CAPACITY: usize = 64;

/// Finished-transaction history records retained per live slot.
///
/// The ring only needs records newer than the oldest live snapshot; anything
/// older is evicted on the spot. This multiplier is headroom for churn while
/// one transaction stays open a long time.
const HISTORY_PER_SLOT: usize = 4;

/// Errors from the bounded transaction pool.
///
/// All pool failures are typed and recoverable: the pool never panics and
/// never grows past its construction-time bounds.
///
/// Shared `Exhausted` postfix, on purpose: every variant is a *typed
/// exhaustion* error for a different bounded resource (transaction slots,
/// history, version records, claimant table, page buffers). Renaming them
/// apart would churn the error surface to satisfy the lint, so the common
/// postfix stays.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MvccError {
    /// No free transaction slot: `active` transactions already live in a pool
    /// of `capacity`. Commit or abort a live transaction and retry.
    #[error("transaction pool exhausted: {active} of {capacity} slots in use")]
    PoolExhausted { active: usize, capacity: usize },

    /// The finished-transaction history ring is full and its oldest record is
    /// still visible to a live snapshot, so it cannot be evicted. Commit the
    /// oldest live transaction (or raise the pool capacity) and retry.
    #[error("transaction history exhausted: {len} records retained (capacity {capacity})")]
    HistoryExhausted { len: usize, capacity: usize },

    /// No free version slot: `used` uncommitted tuple versions are already
    /// claimed in a pool of `capacity`. Commit or abort the writing
    /// transaction(s) and retry.
    #[error("version pool exhausted: {used} of {capacity} version slots in use")]
    VersionPoolExhausted { used: usize, capacity: usize },

    /// No free claimant slot: `active` distinct transactions already hold
    /// version claims in a table of `capacity`. End one of those
    /// transactions and retry. The engine sizes this table from its
    /// transaction-pool capacity, so it cannot trigger spuriously there.
    #[error("version claimant table exhausted: {active} of {capacity} claimant slots in use")]
    VersionClaimantsExhausted { active: usize, capacity: usize },

    /// No free page working buffer: `used` buffers are already checked out
    /// of a pool of `capacity`. This indicates unexpected re-entrant page
    /// work; it is a typed error, never a panic.
    #[error("version buffer pool exhausted: {used} of {capacity} buffers in use")]
    VersionBuffersExhausted { used: usize, capacity: usize },
}

/// One live-transaction slot.
///
/// Slots are pre-allocated at pool construction and never move; `begin` fills
/// a free slot, `commit`/`abort` drain it into the history ring and return the
/// slot to the free list.
#[derive(Debug, Clone, Copy)]
struct TxnSlot {
    /// Whether this slot currently holds a live transaction.
    occupied: bool,
    /// The live transaction's ID (meaningful only if `occupied`).
    txn_id: TransactionId,
    /// LSN at the moment the transaction began (its snapshot's basis).
    snapshot_lsn: Lsn,
    /// Visibility horizon stamped at begin (see [`TransactionSnapshot`]).
    horizon: TransactionId,
}

impl TxnSlot {
    /// A free slot.
    fn empty() -> Self {
        Self {
            occupied: false,
            txn_id: TransactionId::new(0),
            snapshot_lsn: Lsn::new(0),
            horizon: TransactionId::new(u64::MAX),
        }
    }
}

/// A finished transaction's visibility record.
///
/// Retained in the history ring so snapshots taken while the transaction was
/// alive can still answer "was T active at my snapshot?" after T is gone.
#[derive(Debug, Clone, Copy)]
struct HistoryRecord {
    /// The finished transaction's ID.
    txn_id: TransactionId,
    /// LSN at the moment the transaction began.
    begin_lsn: Lsn,
    /// LSN at the moment the transaction committed; `None` if it aborted.
    commit_lsn: Option<Lsn>,
}

/// Bounded, caller-owned pool of MVCC transaction records.
///
/// Owns every byte it will ever need: `capacity` live slots, a free-list
/// stack, and a history ring sized at `capacity * HISTORY_PER_SLOT`. After
/// construction, `begin`, `commit`, `abort`, and all read queries allocate
/// nothing.
#[derive(Debug)]
pub struct TxnPool {
    /// Live-transaction slots; `slots[i]` is live iff `i` is not in `free`.
    slots: Box<[TxnSlot]>,
    /// Stack of free slot indices.
    free: Vec<usize>,
    /// Finished-transaction records, oldest first; bounded by
    /// `history_capacity`.
    history: VecDeque<HistoryRecord>,
    /// Maximum history records retained.
    history_capacity: usize,
}

impl TxnPool {
    /// Creates a pool for at most `capacity` concurrently live transactions.
    ///
    /// All memory — slots, free list, and history ring — is pre-allocated
    /// here. A `capacity` of zero is clamped to one.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let history_capacity = capacity.saturating_mul(HISTORY_PER_SLOT).max(1);
        Self {
            slots: vec![TxnSlot::empty(); capacity].into_boxed_slice(),
            free: (0..capacity).rev().collect(),
            history: VecDeque::with_capacity(history_capacity),
            history_capacity,
        }
    }

    /// Maximum concurrently live transactions.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Number of currently live transactions.
    pub fn active_count(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    /// Number of finished-transaction records currently retained.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Begins a transaction: claims a free slot and returns its snapshot.
    ///
    /// The snapshot is `Copy` and carries no heap data — liveness questions
    /// are answered against the pool, never against a per-snapshot copy.
    ///
    /// # Errors
    ///
    /// Returns [`MvccError::PoolExhausted`] if every slot is live.
    pub fn begin(
        &mut self,
        txn_id: TransactionId,
        lsn: Lsn,
        horizon: TransactionId,
    ) -> Result<crate::mvcc::TransactionSnapshot, MvccError> {
        // Refresh semantics match the old table's `insert` overwrite: a
        // begin for an already-live ID restarts its record in place.
        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.occupied && slot.txn_id == txn_id)
        {
            slot.snapshot_lsn = lsn;
            slot.horizon = horizon;
        } else {
            let index = self.free.pop().ok_or(MvccError::PoolExhausted {
                active: self.active_count(),
                capacity: self.capacity(),
            })?;
            self.slots[index] = TxnSlot {
                occupied: true,
                txn_id,
                snapshot_lsn: lsn,
                horizon,
            };
        }
        Ok(crate::mvcc::TransactionSnapshot::new(txn_id, lsn).with_horizon(horizon))
    }

    /// Commits a transaction: drains its slot into the history ring and frees
    /// the slot.
    ///
    /// `commit_lsn` must be the WAL's current LSN at commit time — a stamp no
    /// earlier than any snapshot taken while the transaction was alive. It is
    /// what lets later snapshots tell "committed before my snapshot" apart
    /// from "committed after it" (Snapshot Isolation); stamping the begin LSN
    /// instead would wrongly expose the transaction's versions to concurrent
    /// snapshots.
    ///
    /// Committing an unknown transaction is a no-op, matching the old table.
    ///
    /// # Errors
    ///
    /// Returns [`MvccError::HistoryExhausted`] if the history ring is full
    /// and its oldest record is still visible to a live snapshot.
    pub fn commit(&mut self, txn_id: TransactionId, commit_lsn: Lsn) -> Result<(), MvccError> {
        self.finish(txn_id, Some(commit_lsn))
    }

    /// Aborts a transaction: drains its slot into the history ring (marked
    /// aborted) and frees the slot.
    ///
    /// Aborting an unknown transaction is a no-op, matching the old table.
    ///
    /// # Errors
    ///
    /// Returns [`MvccError::HistoryExhausted`] if the history ring is full
    /// and its oldest record is still visible to a live snapshot.
    pub fn abort(&mut self, txn_id: TransactionId) -> Result<(), MvccError> {
        self.finish(txn_id, None)
    }

    /// Dry-run of [`TxnPool::commit`]/[`TxnPool::abort`]'s history push:
    /// reports whether finishing `txn_id` right now would fail with
    /// [`MvccError::HistoryExhausted`], without mutating anything.
    ///
    /// The engine calls this *before* making a commit durable, so the
    /// in-memory finish that follows the durable WAL commit record cannot
    /// fail: after the commit point, no typed error may intervene between
    /// the WAL and the pool — they must never disagree about whether the
    /// transaction committed. This check-then-act is sound because the
    /// engine drives the pool single-threaded: nothing can finish a
    /// transaction between the check and the finish.
    ///
    /// Finishing an unknown transaction is a no-op, so this returns `Ok`
    /// for IDs with no live slot, matching [`TxnPool::commit`].
    pub fn can_finish(&self, txn_id: TransactionId) -> Result<(), MvccError> {
        if !self
            .slots
            .iter()
            .any(|slot| slot.occupied && slot.txn_id == txn_id)
        {
            return Ok(());
        }
        // Mirror `push_history`'s eviction without mutating: count how many
        // records `evict_history` would drop from the front (eviction stops
        // at the first record no live snapshot can release). Like the real
        // finish, the check runs while the finishing transaction is still
        // live, so the horizon used here matches exactly.
        let oldest = self.oldest_active_lsn();
        let mut evictable = 0;
        for record in self.history.iter() {
            let releasable = match record.commit_lsn {
                None => true,
                Some(commit_lsn) => oldest.is_none_or(|old| commit_lsn <= old),
            };
            if releasable {
                evictable += 1;
            } else {
                break;
            }
        }
        if self.history.len() - evictable >= self.history_capacity {
            return Err(MvccError::HistoryExhausted {
                len: self.history.len(),
                capacity: self.history_capacity,
            });
        }
        Ok(())
    }

    /// Drains a live slot into the history ring; `commit_lsn` is `Some` for a
    /// commit and `None` for an abort.
    fn finish(&mut self, txn_id: TransactionId, commit_lsn: Option<Lsn>) -> Result<(), MvccError> {
        let index = match self
            .slots
            .iter()
            .position(|slot| slot.occupied && slot.txn_id == txn_id)
        {
            Some(index) => index,
            None => return Ok(()),
        };
        let slot = self.slots[index];
        self.push_history(HistoryRecord {
            txn_id,
            begin_lsn: slot.snapshot_lsn,
            commit_lsn,
        })?;
        self.slots[index] = TxnSlot::empty();
        self.free.push(index);
        Ok(())
    }

    /// Pushes a history record, evicting provably-invisible records first.
    fn push_history(&mut self, record: HistoryRecord) -> Result<(), MvccError> {
        self.evict_history();
        if self.history.len() >= self.history_capacity {
            return Err(MvccError::HistoryExhausted {
                len: self.history.len(),
                capacity: self.history_capacity,
            });
        }
        self.history.push_back(record);
        Ok(())
    }

    /// Evicts history records no live snapshot can still observe, oldest first.
    ///
    /// A record is evictable when every live snapshot began at or after the
    /// record stopped being "active":
    /// - Aborted records are always evictable: an aborted transaction's
    ///   versions are invisible through the committed set regardless of what
    ///   `is_active_at` would report, and its deletes never hide a version
    ///   (the `!committed.contains(xmax)` check fires first).
    /// - A committed record with `commit_lsn <= oldest live snapshot LSN`
    ///   was already committed when every live snapshot was taken, so the
    ///   correct liveness answer for it is "not active" — exactly what the
    ///   post-eviction fallback reports.
    fn evict_history(&mut self) {
        let oldest = self.oldest_active_lsn();
        while let Some(front) = self.history.front() {
            let evictable = match front.commit_lsn {
                None => true,
                Some(commit_lsn) => oldest.is_none_or(|o| commit_lsn <= o),
            };
            if evictable {
                self.history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Returns the snapshot for a live transaction, if it is still live.
    pub fn get_snapshot(&self, txn_id: TransactionId) -> Option<crate::mvcc::TransactionSnapshot> {
        self.slots
            .iter()
            .find(|slot| slot.occupied && slot.txn_id == txn_id)
            .map(|slot| {
                crate::mvcc::TransactionSnapshot::new(slot.txn_id, slot.snapshot_lsn)
                    .with_horizon(slot.horizon)
            })
    }

    /// Was `txn_id` still running (uncommitted) at the snapshot taken at
    /// `snapshot_lsn`?
    ///
    /// Consults live slots first, then the history ring; anything found in
    /// neither was either never begun or evicted as provably invisible to
    /// every live snapshot — both correctly report "not active".
    ///
    /// Aborted transactions report "not active": their created versions are
    /// invisible via the committed set, and their deletes never hide a
    /// version, so no visibility path depends on the "was active" answer for
    /// them.
    pub fn is_active_at(&self, txn_id: TransactionId, snapshot_lsn: Lsn) -> bool {
        if self
            .slots
            .iter()
            .any(|slot| slot.occupied && slot.txn_id == txn_id)
        {
            return true;
        }
        self.history
            .iter()
            .find(|record| record.txn_id == txn_id)
            .is_some_and(|record| {
                record.begin_lsn <= snapshot_lsn
                    && record
                        .commit_lsn
                        .is_some_and(|commit_lsn| commit_lsn > snapshot_lsn)
            })
    }

    /// The oldest LSN among live transactions' snapshots.
    ///
    /// Drives history eviction and (via the engine) garbage collection: any
    /// version deleted before this LSN is invisible to every live and future
    /// transaction.
    pub fn oldest_active_lsn(&self) -> Option<Lsn> {
        self.slots
            .iter()
            .filter(|slot| slot.occupied)
            .map(|slot| slot.snapshot_lsn)
            .min()
    }

    /// Was `txn_id`'s commit settled before `lsn` — i.e., is every snapshot
    /// taken at or after `lsn` guaranteed to observe the transaction as
    /// already committed (never concurrent)?
    ///
    /// This is the predicate garbage collection needs: a version deleted
    /// by `txn_id` is dead exactly when the deleter's commit settled
    /// before the oldest live snapshot's LSN. Comparing the transaction
    /// *ID* against an LSN instead would be unsound — IDs and LSNs are
    /// different sequences (IDs stay tiny while the LSN grows with every
    /// WAL record).
    ///
    /// Returns:
    /// * `Some(true)` — the transaction committed with `commit_lsn <= lsn`;
    /// * `Some(false)` — the transaction is still live, or committed after
    ///   `lsn`, or its history record marks it aborted;
    /// * `None` — the transaction went through no retained record: its
    ///   history entry was evicted, or it committed before this pool
    ///   existed (post-reopen recovery). Eviction only drops records with
    ///   `commit_lsn <=` the oldest live snapshot's LSN at eviction time,
    ///   and that horizon only moves forward; a pre-pool commit predates
    ///   every snapshot this pool can ever take. Either way the commit
    ///   settled before `lsn`, so GC treats `None` as settled.
    pub fn committed_before(&self, txn_id: TransactionId, lsn: Lsn) -> Option<bool> {
        if self
            .slots
            .iter()
            .any(|slot| slot.occupied && slot.txn_id == txn_id)
        {
            return Some(false);
        }
        self.history
            .iter()
            .find(|record| record.txn_id == txn_id)
            .map(|record| {
                // An aborted record reports "not settled": if the caller
                // also believes the transaction committed, the inputs
                // contradict each other and GC must take the conservative
                // side.
                record
                    .commit_lsn
                    .is_some_and(|commit_lsn| commit_lsn <= lsn)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lsn(value: u64) -> Lsn {
        Lsn::new(value)
    }

    fn test_txn(value: u64) -> TransactionId {
        TransactionId::new(value)
    }

    #[test]
    fn test_pool_new_preallocates_capacity() {
        let pool = TxnPool::new(8);
        assert_eq!(pool.capacity(), 8);
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.history_len(), 0);
    }

    #[test]
    fn test_pool_zero_capacity_clamped() {
        let pool = TxnPool::new(0);
        assert_eq!(pool.capacity(), 1);
    }

    #[test]
    fn test_pool_begin_returns_copy_snapshot() {
        let mut pool = TxnPool::new(8);
        let snapshot = pool.begin(test_txn(1), test_lsn(100), test_txn(2)).unwrap();
        assert_eq!(snapshot.txn_id, test_txn(1));
        assert_eq!(snapshot.snapshot_lsn, test_lsn(100));
        assert_eq!(snapshot.horizon, test_txn(2));
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn test_pool_begin_never_allocates_beyond_construction() {
        // Begin/commit churn must not grow any allocation: run more cycles
        // than the capacity and assert the pool's footprint is unchanged.
        let mut pool = TxnPool::new(4);
        for i in 1..=20u64 {
            let id = test_txn(i);
            pool.begin(id, test_lsn(100 + i), test_txn(u64::MAX))
                .unwrap();
            // Commit LSN just past the begin LSN, as the WAL would stamp it.
            pool.commit(id, test_lsn(100 + i + 1)).unwrap();
        }
        assert_eq!(pool.active_count(), 0);
        // History holds the finished records, bounded by the ring.
        assert!(pool.history_len() <= 4 * HISTORY_PER_SLOT);
    }

    #[test]
    fn test_pool_exhausted_returns_typed_error() {
        let mut pool = TxnPool::new(2);
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        pool.begin(test_txn(2), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        let err = pool
            .begin(test_txn(3), test_lsn(100), test_txn(u64::MAX))
            .unwrap_err();
        assert_eq!(
            err,
            MvccError::PoolExhausted {
                active: 2,
                capacity: 2
            }
        );
    }

    #[test]
    fn test_pool_commit_frees_slot_for_reuse() {
        let mut pool = TxnPool::new(1);
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        assert!(
            pool.begin(test_txn(2), test_lsn(100), test_txn(u64::MAX))
                .is_err()
        );
        pool.commit(test_txn(1), test_lsn(150)).unwrap();
        // Slot is reusable; the new begin succeeds.
        pool.begin(test_txn(2), test_lsn(200), test_txn(u64::MAX))
            .unwrap();
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn test_pool_commit_unknown_is_noop() {
        let mut pool = TxnPool::new(4);
        assert!(pool.commit(test_txn(999), test_lsn(1)).is_ok());
        assert!(pool.abort(test_txn(999)).is_ok());
    }

    #[test]
    fn test_pool_get_snapshot_live_and_gone() {
        let mut pool = TxnPool::new(4);
        pool.begin(test_txn(1), test_lsn(100), test_txn(5)).unwrap();
        let snapshot = pool.get_snapshot(test_txn(1)).unwrap();
        assert_eq!(snapshot.txn_id, test_txn(1));
        assert_eq!(snapshot.horizon, test_txn(5));
        pool.commit(test_txn(1), test_lsn(150)).unwrap();
        assert!(pool.get_snapshot(test_txn(1)).is_none());
    }

    #[test]
    fn test_pool_is_active_at_live_transaction() {
        let mut pool = TxnPool::new(4);
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        assert!(pool.is_active_at(test_txn(1), test_lsn(100)));
        assert!(pool.is_active_at(test_txn(1), test_lsn(999)));
    }

    #[test]
    fn test_pool_is_active_at_committed_before_snapshot() {
        let mut pool = TxnPool::new(4);
        // T1 begins at 100 and commits at 150.
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        pool.commit(test_txn(1), test_lsn(150)).unwrap();
        // A snapshot taken at 200 sees T1 as no longer active.
        assert!(!pool.is_active_at(test_txn(1), test_lsn(200)));
    }

    #[test]
    fn test_pool_is_active_at_committed_after_snapshot() {
        let mut pool = TxnPool::new(4);
        // T1 begins at 100. T2's snapshot is taken at 150 while T1 is live.
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        let t2 = pool
            .begin(test_txn(2), test_lsn(150), test_txn(u64::MAX))
            .unwrap();
        // T1 commits at 180, after T2's snapshot: the history ring still
        // reports T1 as active-at-150 for T2's snapshot.
        pool.commit(test_txn(1), test_lsn(180)).unwrap();
        assert!(pool.is_active_at(test_txn(1), t2.snapshot_lsn));
        // ...but not for a later snapshot.
        assert!(!pool.is_active_at(test_txn(1), test_lsn(1000)));
    }

    #[test]
    fn test_pool_is_active_at_aborted_reports_not_active() {
        let mut pool = TxnPool::new(4);
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        let t2 = pool
            .begin(test_txn(2), test_lsn(150), test_txn(u64::MAX))
            .unwrap();
        pool.abort(test_txn(1)).unwrap();
        assert!(!pool.is_active_at(test_txn(1), t2.snapshot_lsn));
    }

    #[test]
    fn test_pool_is_active_at_unknown_reports_not_active() {
        let pool = TxnPool::new(4);
        assert!(!pool.is_active_at(test_txn(999), test_lsn(100)));
    }

    #[test]
    fn test_pool_oldest_active_lsn() {
        let mut pool = TxnPool::new(4);
        assert_eq!(pool.oldest_active_lsn(), None);
        pool.begin(test_txn(1), test_lsn(300), test_txn(u64::MAX))
            .unwrap();
        pool.begin(test_txn(2), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        assert_eq!(pool.oldest_active_lsn(), Some(test_lsn(100)));
        pool.commit(test_txn(2), test_lsn(400)).unwrap();
        assert_eq!(pool.oldest_active_lsn(), Some(test_lsn(300)));
    }

    #[test]
    fn test_pool_history_evicts_committed_before_oldest_snapshot() {
        let mut pool = TxnPool::new(2);
        // T1 lives long at LSN 100; T2..T5 churn through commits.
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        for i in 2..=5u64 {
            pool.begin(test_txn(i), test_lsn(100 + i), test_txn(u64::MAX))
                .unwrap();
            pool.commit(test_txn(i), test_lsn(200 + i)).unwrap();
        }
        // T1 is still live at 100, so T2..T5 (committed after 100) stay.
        assert_eq!(pool.history_len(), 4);
        // Once T1 commits, every record is evictable on the next finish.
        pool.commit(test_txn(1), test_lsn(500)).unwrap();
        pool.begin(test_txn(6), test_lsn(600), test_txn(u64::MAX))
            .unwrap();
        pool.commit(test_txn(6), test_lsn(610)).unwrap();
        // Only T6's record survives: everything older committed before
        // the oldest live snapshot (none live => +infinity).
        assert_eq!(pool.history_len(), 1);
    }

    #[test]
    fn test_pool_history_exhausted_is_typed() {
        let mut pool = TxnPool::new(2);
        // Shrink the ring to force the error path deterministically.
        pool.history_capacity = 1;
        // T1 stays live forever at LSN 100: nothing committed after 100
        // can ever be evicted while it lives.
        pool.begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        // T2 aborts at 200: aborted records evict freely, so this fits.
        pool.begin(test_txn(2), test_lsn(200), test_txn(u64::MAX))
            .unwrap();
        pool.abort(test_txn(2)).unwrap();
        assert_eq!(pool.history_len(), 1);
        // T3 commits at 300: the aborted record evicts, T3's record stays.
        pool.begin(test_txn(3), test_lsn(300), test_txn(u64::MAX))
            .unwrap();
        pool.commit(test_txn(3), test_lsn(310)).unwrap();
        assert_eq!(pool.history_len(), 1);
        // T4 commits at 400: ring full, oldest record (T3, committed at 310)
        // is newer than the oldest live snapshot (100) -> not evictable.
        pool.begin(test_txn(4), test_lsn(400), test_txn(u64::MAX))
            .unwrap();
        let err = pool.commit(test_txn(4), test_lsn(410)).unwrap_err();
        assert_eq!(
            err,
            MvccError::HistoryExhausted {
                len: 1,
                capacity: 1
            }
        );
        // The failed commit leaves T4's slot live (no silent loss).
        assert!(pool.get_snapshot(test_txn(4)).is_some());
    }

    #[test]
    fn test_pool_begin_existing_id_refreshes_in_place() {
        let mut pool = TxnPool::new(2);
        pool.begin(test_txn(1), test_lsn(100), test_txn(10))
            .unwrap();
        // Re-begin of a live ID refreshes its record (old table semantics).
        let snapshot = pool
            .begin(test_txn(1), test_lsn(200), test_txn(20))
            .unwrap();
        assert_eq!(snapshot.snapshot_lsn, test_lsn(200));
        assert_eq!(snapshot.horizon, test_txn(20));
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn test_pool_abort_unknown_is_noop() {
        let mut pool = TxnPool::new(2);
        // Aborting a transaction that was never begun frees nothing and
        // records nothing: the pool is unchanged.
        assert!(pool.abort(test_txn(999)).is_ok());
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.history_len(), 0);
    }

    #[test]
    fn test_pool_mvcc_error_converts_to_typed_storage_error() {
        use relvar_core::storage_engine::StorageError;

        let pool_err = MvccError::PoolExhausted {
            active: 3,
            capacity: 3,
        };
        assert!(
            matches!(
                StorageError::from(pool_err),
                StorageError::TransactionPoolExhausted {
                    active: 3,
                    capacity: 3
                }
            ),
            "pool exhaustion must stay typed across the engine boundary"
        );

        let history_err = MvccError::HistoryExhausted {
            len: 8,
            capacity: 8,
        };
        assert!(
            matches!(
                StorageError::from(history_err),
                StorageError::TransactionHistoryExhausted {
                    len: 8,
                    capacity: 8
                }
            ),
            "history exhaustion must stay typed across the engine boundary"
        );
    }
}
