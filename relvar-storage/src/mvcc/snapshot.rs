//! # MVCC transaction snapshots
//!
//! A [`TransactionSnapshot`] is a point-in-time view of the database as seen
//! by one transaction. It is a plain `Copy` value — `{txn_id, snapshot_lsn,
//! horizon}` — carrying no heap data. Liveness questions ("was transaction T
//! still running when this snapshot was taken?") are answered against the
//! [`TxnPool`](crate::mvcc::TxnPool), never against a per-snapshot copy of
//! the active set, so taking a snapshot allocates nothing.
//!
//! ## Visibility rules (Snapshot Isolation)
//!
//! A tuple version is visible to a snapshot when:
//!
//! 1. It was created by a transaction that had committed before the snapshot
//!    was taken (or by the snapshotting transaction itself), and
//! 2. It was not deleted by a transaction that had committed before the
//!    snapshot was taken (deletes by the snapshotting transaction itself hide
//!    the version from it).
//!
//! The `horizon` is the next transaction ID the generator would hand out at
//! snapshot time: any transaction with an ID at or above the horizon began
//! after the snapshot and is invisible to it, full stop.
//!
//! ## TTM Compliance
//!
//! Snapshots are an internal storage-layer mechanism (Physical Data
//! Independence). The logical layer never sees transaction IDs or LSNs.

use crate::mvcc::TxnPool;
use crate::wal::{Lsn, TransactionId};

/// A point-in-time view of the database for one transaction.
///
/// `Copy` and allocation-free: construct it from the pool
/// ([`TxnPool::begin`]/[`TxnPool::get_snapshot`]) and query liveness through
/// [`TransactionSnapshot::is_active`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionSnapshot {
    /// The transaction that owns this snapshot.
    pub txn_id: TransactionId,
    /// LSN at the moment the snapshot was taken.
    pub snapshot_lsn: Lsn,
    /// Transactions with IDs at or above this began after this snapshot was
    /// taken, so their versions are invisible to it.
    pub horizon: TransactionId,
}

impl TransactionSnapshot {
    /// Creates a snapshot for `txn_id` taken at `snapshot_lsn`.
    ///
    /// The horizon defaults to `u64::MAX` (every begun transaction is
    /// potentially visible, subject to the commit/active rules); stamp the
    /// real horizon with [`TransactionSnapshot::with_horizon`] when the
    /// generator is handy.
    pub fn new(txn_id: TransactionId, snapshot_lsn: Lsn) -> Self {
        Self {
            txn_id,
            snapshot_lsn,
            horizon: TransactionId::new(u64::MAX),
        }
    }

    /// Stamps the visibility horizon: the next transaction ID the generator
    /// would hand out at snapshot time.
    pub fn with_horizon(mut self, horizon: TransactionId) -> Self {
        self.horizon = horizon;
        self
    }

    /// Was `txn_id` still running (uncommitted) when this snapshot was taken?
    ///
    /// The snapshotting transaction itself never counts as active: its own
    /// versions are visible to it through the committed path, not the active
    /// path.
    pub fn is_active(&self, txn_id: TransactionId, pool: &TxnPool) -> bool {
        txn_id != self.txn_id && pool.is_active_at(txn_id, self.snapshot_lsn)
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
    fn test_snapshot_new_defaults_to_open_horizon() {
        let snapshot = TransactionSnapshot::new(test_txn(1), test_lsn(100));
        assert_eq!(snapshot.txn_id, test_txn(1));
        assert_eq!(snapshot.snapshot_lsn, test_lsn(100));
        assert_eq!(snapshot.horizon, test_txn(u64::MAX));
    }

    #[test]
    fn test_snapshot_with_horizon() {
        let snapshot =
            TransactionSnapshot::new(test_txn(1), test_lsn(100)).with_horizon(test_txn(42));
        assert_eq!(snapshot.horizon, test_txn(42));
    }

    #[test]
    fn test_snapshot_is_copy() {
        let snapshot = TransactionSnapshot::new(test_txn(1), test_lsn(100));
        let copy = snapshot;
        assert_eq!(snapshot, copy);
    }

    #[test]
    fn test_snapshot_is_active_delegates_to_pool() {
        let mut pool = TxnPool::new(4);
        pool.begin(test_txn(2), test_lsn(50), test_txn(u64::MAX))
            .unwrap();
        let snapshot = pool
            .begin(test_txn(1), test_lsn(100), test_txn(u64::MAX))
            .unwrap();
        // T2 is live in the pool: active at T1's snapshot.
        assert!(snapshot.is_active(test_txn(2), &pool));
        // T1 never counts itself as active.
        assert!(!snapshot.is_active(test_txn(1), &pool));
        // Unknown transactions are not active.
        assert!(!snapshot.is_active(test_txn(999), &pool));
        // T2 commits at 150, after the snapshot at 100: it was still running
        // when the snapshot was taken, so it counts as active-at-100.
        pool.commit(test_txn(2), test_lsn(150)).unwrap();
        assert!(snapshot.is_active(test_txn(2), &pool));
        // A later snapshot taken after the commit sees T2 as inactive.
        let later = pool
            .begin(test_txn(3), test_lsn(200), test_txn(u64::MAX))
            .unwrap();
        assert!(!later.is_active(test_txn(2), &pool));
    }
}
