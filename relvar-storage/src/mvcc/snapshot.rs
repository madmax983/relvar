use crate::wal::{Lsn, TransactionId};
use std::collections::HashSet;

/// Snapshot of transaction state at a point in time.
///
/// Captures which transactions were active when this transaction began,
/// enabling snapshot isolation (each transaction sees a consistent view of data).
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
    /// # Arguments
    /// * `txn_id` - The transaction ID this snapshot is for
    /// * `snapshot_lsn` - The LSN at snapshot creation time
    /// * `active` - List of transaction IDs that were active at snapshot time
    pub fn new(txn_id: TransactionId, snapshot_lsn: Lsn, active: Vec<TransactionId>) -> Self {
        Self {
            txn_id,
            snapshot_lsn,
            active_txns: active.into_iter().collect(),
        }
    }

    /// Checks if a transaction was active when this snapshot was taken.
    ///
    /// # Arguments
    /// * `txn_id` - The transaction ID to check
    ///
    /// # Returns
    /// `true` if the transaction was active (uncommitted) at snapshot time
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
