//! # Transaction Snapshots
//!
//! Provides the core abstraction for Snapshot Isolation. A snapshot represents
//! a point in time view of the database. When a transaction starts, it receives
//! a snapshot that defines what data versions it is allowed to see.

use crate::wal::{Lsn, TransactionId};
use std::collections::HashSet;

/// Snapshot of transaction state at a point in time.
///
/// A `TransactionSnapshot` is created when a transaction begins. It captures:
/// 1. The transaction's own ID.
/// 2. The Log Sequence Number (LSN) at the time it started.
/// 3. The exact set of *other* transactions that were running (uncommitted) at that moment.
///
/// This structure is strictly read-only after creation and is passed to the visibility
/// checker to ensure that any changes made by the `active_txns` are hidden from this transaction.
///
/// ## Examples
///
/// ```
/// use relvar_storage::mvcc::TransactionSnapshot;
/// use relvar_storage::wal::{Lsn, TransactionId};
///
/// let my_txn = TransactionId::new(42);
/// let current_lsn = Lsn::new(100);
/// // Imagine transactions 40 and 41 are currently running
/// let active = vec![TransactionId::new(40), TransactionId::new(41)];
///
/// let snapshot = TransactionSnapshot::new(my_txn, current_lsn, active);
///
/// // We know that txn 40 was active when we started, so we must NOT see its changes.
/// assert!(snapshot.is_active(TransactionId::new(40)));
///
/// // Txn 39 is NOT in the active list, meaning it committed before we started.
/// // We ARE allowed to see its changes.
/// assert!(!snapshot.is_active(TransactionId::new(39)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionSnapshot {
    /// The transaction ID this snapshot belongs to
    pub txn_id: TransactionId,
    /// LSN at the time this snapshot was taken
    pub snapshot_lsn: Lsn,
    /// Set of transactions that were active (uncommitted) when snapshot was taken
    pub active_txns: HashSet<TransactionId>,
}

impl TransactionSnapshot {
    /// Creates a new transaction snapshot.
    ///
    /// This is typically called exclusively by the `ActiveTransactionTable` when
    /// a new transaction is registered.
    ///
    /// # Arguments
    /// * `txn_id` - The transaction ID this snapshot is for.
    /// * `snapshot_lsn` - The LSN at the exact moment the snapshot was created.
    /// * `active` - A list of transaction IDs that are currently uncommitted.
    ///
    /// ## Examples
    ///
    /// ```
    /// use relvar_storage::mvcc::TransactionSnapshot;
    /// use relvar_storage::wal::{Lsn, TransactionId};
    ///
    /// let snapshot = TransactionSnapshot::new(
    ///     TransactionId::new(10),
    ///     Lsn::new(500),
    ///     vec![TransactionId::new(8), TransactionId::new(9)]
    /// );
    ///
    /// assert_eq!(snapshot.txn_id, TransactionId::new(10));
    /// ```
    pub fn new(txn_id: TransactionId, snapshot_lsn: Lsn, active: Vec<TransactionId>) -> Self {
        Self {
            txn_id,
            snapshot_lsn,
            active_txns: active.into_iter().collect(),
        }
    }

    /// Checks if a given transaction was active (uncommitted) when this snapshot was created.
    ///
    /// This is the primary method used by the visibility engine. If a tuple version
    /// was created or deleted by a transaction that returns `true` here, that version's
    /// state is considered "in flux" and must be ignored by the transaction holding this snapshot.
    ///
    /// # Arguments
    /// * `txn_id` - The transaction ID to check.
    ///
    /// # Returns
    /// `true` if the transaction was active (uncommitted) at snapshot time, `false` otherwise.
    ///
    /// ## Examples
    ///
    /// ```
    /// use relvar_storage::mvcc::TransactionSnapshot;
    /// use relvar_storage::wal::{Lsn, TransactionId};
    ///
    /// let t1 = TransactionId::new(1);
    /// let t2 = TransactionId::new(2);
    ///
    /// let snapshot = TransactionSnapshot::new(t2, Lsn::new(100), vec![t1]);
    ///
    /// assert!(snapshot.is_active(t1)); // T1 was running
    /// assert!(!snapshot.is_active(TransactionId::new(0))); // T0 was not running
    /// ```
    pub fn is_active(&self, txn_id: TransactionId) -> bool {
        self.active_txns.contains(&txn_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test LSN
    fn test_lsn(value: u64) -> Lsn {
        Lsn::new(value)
    }

    // Helper to create test TransactionId
    fn test_txn(value: u64) -> TransactionId {
        TransactionId::new(value)
    }

    #[test]
    fn test_snapshot_creation_captures_lsn() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(2), test_txn(3)];

        let snapshot = TransactionSnapshot::new(txn_id, lsn, active);

        assert_eq!(snapshot.txn_id, txn_id);
        assert_eq!(snapshot.snapshot_lsn, lsn);
    }

    #[test]
    fn test_snapshot_tracks_active_txns() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(2), test_txn(3)];

        let snapshot = TransactionSnapshot::new(txn_id, lsn, active.clone());

        assert_eq!(snapshot.active_txns.len(), 2);
        assert!(snapshot.active_txns.contains(&test_txn(2)));
        assert!(snapshot.active_txns.contains(&test_txn(3)));
    }

    #[test]
    fn test_snapshot_is_active_check() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(2), test_txn(3)];

        let snapshot = TransactionSnapshot::new(txn_id, lsn, active);

        assert!(snapshot.is_active(test_txn(2)));
        assert!(snapshot.is_active(test_txn(3)));
        assert!(!snapshot.is_active(test_txn(4)));
    }

    #[test]
    fn test_snapshot_empty_active_list() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![];

        let snapshot = TransactionSnapshot::new(txn_id, lsn, active);

        assert_eq!(snapshot.active_txns.len(), 0);
        assert!(!snapshot.is_active(test_txn(2)));
    }

    #[test]
    fn test_snapshot_does_not_include_self_in_active() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(1), test_txn(2)]; // Including self

        let snapshot = TransactionSnapshot::new(txn_id, lsn, active);

        // Self should be in active_txns set (it's up to caller to exclude if needed)
        assert!(snapshot.active_txns.contains(&test_txn(1)));
        assert!(snapshot.active_txns.contains(&test_txn(2)));
    }

    #[test]
    fn test_snapshot_duplicate_active_txns() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(2), test_txn(2), test_txn(3)]; // Duplicate

        let snapshot = TransactionSnapshot::new(txn_id, lsn, active);

        // HashSet should deduplicate
        assert_eq!(snapshot.active_txns.len(), 2);
        assert!(snapshot.active_txns.contains(&test_txn(2)));
        assert!(snapshot.active_txns.contains(&test_txn(3)));
    }

    #[test]
    fn test_snapshot_clone() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(2), test_txn(3)];

        let snapshot1 = TransactionSnapshot::new(txn_id, lsn, active);
        let snapshot2 = snapshot1.clone();

        assert_eq!(snapshot1, snapshot2);
        assert_eq!(snapshot1.txn_id, snapshot2.txn_id);
        assert_eq!(snapshot1.snapshot_lsn, snapshot2.snapshot_lsn);
        assert_eq!(snapshot1.active_txns, snapshot2.active_txns);
    }

    #[test]
    fn test_snapshot_equality() {
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);
        let active = vec![test_txn(2), test_txn(3)];

        let snapshot1 = TransactionSnapshot::new(txn_id, lsn, active.clone());
        let snapshot2 = TransactionSnapshot::new(txn_id, lsn, active);

        assert_eq!(snapshot1, snapshot2);
    }

    #[test]
    fn test_snapshot_inequality_different_txn_id() {
        let lsn = test_lsn(100);
        let active = vec![test_txn(2)];

        let snapshot1 = TransactionSnapshot::new(test_txn(1), lsn, active.clone());
        let snapshot2 = TransactionSnapshot::new(test_txn(2), lsn, active);

        assert_ne!(snapshot1, snapshot2);
    }

    #[test]
    fn test_snapshot_inequality_different_lsn() {
        let txn_id = test_txn(1);
        let active = vec![test_txn(2)];

        let snapshot1 = TransactionSnapshot::new(txn_id, test_lsn(100), active.clone());
        let snapshot2 = TransactionSnapshot::new(txn_id, test_lsn(200), active);

        assert_ne!(snapshot1, snapshot2);
    }
}
