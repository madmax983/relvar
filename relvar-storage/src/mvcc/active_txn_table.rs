// Active Transaction Table - tracks concurrent transactions
// Will be implemented in Phase 1.2

use crate::mvcc::TransactionSnapshot;
use crate::wal::{Lsn, TransactionId};
use std::collections::HashMap;

/// Tracks all active (uncommitted) transactions.
#[derive(Debug)]
pub struct ActiveTransactionTable {
    transactions: HashMap<TransactionId, TransactionSnapshot>,
    current_lsn: Lsn,
}

impl ActiveTransactionTable {
    /// Creates a new empty transaction table.
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            current_lsn: Lsn::new(0),
        }
    }
}

impl Default for ActiveTransactionTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveTransactionTable {
    /// Begin a new transaction and create its snapshot.
    ///
    /// # Arguments
    /// * `txn_id` - The new transaction ID
    /// * `lsn` - Current LSN at begin time
    ///
    /// # Returns
    /// TransactionSnapshot capturing all currently active transactions
    pub fn begin(&mut self, txn_id: TransactionId, lsn: Lsn) -> TransactionSnapshot {
        // Update current LSN
        self.current_lsn = lsn;

        // Capture all currently active transactions (excluding the new one)
        let active: Vec<TransactionId> = self.transactions.keys().copied().collect();

        // Create snapshot for this transaction
        let snapshot = TransactionSnapshot::new(txn_id, lsn, active);

        // Add to active transaction table
        self.transactions.insert(txn_id, snapshot.clone());

        snapshot
    }

    /// Commit a transaction (remove from active set).
    ///
    /// # Arguments
    /// * `txn_id` - Transaction to commit
    pub fn commit(&mut self, txn_id: TransactionId) {
        self.transactions.remove(&txn_id);
    }

    /// Abort a transaction (remove from active set).
    ///
    /// # Arguments
    /// * `txn_id` - Transaction to abort
    pub fn abort(&mut self, txn_id: TransactionId) {
        self.transactions.remove(&txn_id);
    }

    /// Get the oldest LSN of any active transaction.
    ///
    /// Used by garbage collection to determine which versions are safe to remove.
    ///
    /// # Returns
    /// * `Some(Lsn)` - Oldest LSN if any transactions are active
    /// * `None` - No active transactions
    pub fn oldest_active_lsn(&self) -> Option<Lsn> {
        self.transactions
            .values()
            .map(|snapshot| snapshot.snapshot_lsn)
            .min()
    }

    /// Get the snapshot for a specific transaction.
    ///
    /// # Arguments
    /// * `txn_id` - Transaction ID to lookup
    ///
    /// # Returns
    /// * `Some(&TransactionSnapshot)` - Snapshot if transaction is active
    /// * `None` - Transaction not found (committed/aborted/never existed)
    pub fn get_snapshot(&self, txn_id: TransactionId) -> Option<&TransactionSnapshot> {
        self.transactions.get(&txn_id)
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
    fn test_att_new_is_empty() {
        let att = ActiveTransactionTable::new();
        assert_eq!(att.transactions.len(), 0);
        assert_eq!(att.current_lsn, Lsn::new(0));
    }

    #[test]
    fn test_att_begin_creates_snapshot() {
        let mut att = ActiveTransactionTable::new();
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);

        let snapshot = att.begin(txn_id, lsn);

        assert_eq!(snapshot.txn_id, txn_id);
        assert_eq!(snapshot.snapshot_lsn, lsn);
        assert_eq!(snapshot.active_txns.len(), 0); // No other active txns yet
    }

    #[test]
    fn test_att_begin_adds_to_table() {
        let mut att = ActiveTransactionTable::new();
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);

        att.begin(txn_id, lsn);

        assert_eq!(att.transactions.len(), 1);
        assert!(att.transactions.contains_key(&txn_id));
    }

    #[test]
    fn test_att_concurrent_transactions() {
        let mut att = ActiveTransactionTable::new();

        // T1 begins
        let snapshot1 = att.begin(test_txn(1), test_lsn(100));
        assert_eq!(snapshot1.active_txns.len(), 0);

        // T2 begins (should see T1 as active)
        let snapshot2 = att.begin(test_txn(2), test_lsn(200));
        assert_eq!(snapshot2.active_txns.len(), 1);
        assert!(snapshot2.is_active(test_txn(1)));

        // T3 begins (should see T1 and T2 as active)
        let snapshot3 = att.begin(test_txn(3), test_lsn(300));
        assert_eq!(snapshot3.active_txns.len(), 2);
        assert!(snapshot3.is_active(test_txn(1)));
        assert!(snapshot3.is_active(test_txn(2)));
    }

    #[test]
    fn test_att_commit_removes_txn() {
        let mut att = ActiveTransactionTable::new();
        let txn_id = test_txn(1);

        att.begin(txn_id, test_lsn(100));
        assert_eq!(att.transactions.len(), 1);

        att.commit(txn_id);
        assert_eq!(att.transactions.len(), 0);
    }

    #[test]
    fn test_att_commit_nonexistent_is_noop() {
        let mut att = ActiveTransactionTable::new();

        // Committing non-existent transaction should not panic
        att.commit(test_txn(999));

        assert_eq!(att.transactions.len(), 0);
    }

    #[test]
    fn test_att_abort_removes_txn() {
        let mut att = ActiveTransactionTable::new();
        let txn_id = test_txn(1);

        att.begin(txn_id, test_lsn(100));
        assert_eq!(att.transactions.len(), 1);

        att.abort(txn_id);
        assert_eq!(att.transactions.len(), 0);
    }

    #[test]
    fn test_att_abort_nonexistent_is_noop() {
        let mut att = ActiveTransactionTable::new();

        // Aborting non-existent transaction should not panic
        att.abort(test_txn(999));

        assert_eq!(att.transactions.len(), 0);
    }

    #[test]
    fn test_att_oldest_active_lsn_empty() {
        let att = ActiveTransactionTable::new();
        assert_eq!(att.oldest_active_lsn(), None);
    }

    #[test]
    fn test_att_oldest_active_lsn_single() {
        let mut att = ActiveTransactionTable::new();

        att.begin(test_txn(1), test_lsn(100));

        assert_eq!(att.oldest_active_lsn(), Some(test_lsn(100)));
    }

    #[test]
    fn test_att_oldest_active_lsn_multiple() {
        let mut att = ActiveTransactionTable::new();

        att.begin(test_txn(1), test_lsn(300));
        att.begin(test_txn(2), test_lsn(100)); // Oldest
        att.begin(test_txn(3), test_lsn(200));

        assert_eq!(att.oldest_active_lsn(), Some(test_lsn(100)));
    }

    #[test]
    fn test_att_oldest_active_lsn_after_commit() {
        let mut att = ActiveTransactionTable::new();

        att.begin(test_txn(1), test_lsn(100)); // Oldest
        att.begin(test_txn(2), test_lsn(200));

        att.commit(test_txn(1)); // Remove oldest

        assert_eq!(att.oldest_active_lsn(), Some(test_lsn(200)));
    }

    #[test]
    fn test_att_get_snapshot_exists() {
        let mut att = ActiveTransactionTable::new();
        let txn_id = test_txn(1);
        let lsn = test_lsn(100);

        let created_snapshot = att.begin(txn_id, lsn);
        let retrieved_snapshot = att.get_snapshot(txn_id);

        assert!(retrieved_snapshot.is_some());
        assert_eq!(retrieved_snapshot.unwrap(), &created_snapshot);
    }

    #[test]
    fn test_att_get_snapshot_not_found() {
        let att = ActiveTransactionTable::new();

        assert!(att.get_snapshot(test_txn(999)).is_none());
    }

    #[test]
    fn test_att_get_snapshot_after_commit() {
        let mut att = ActiveTransactionTable::new();
        let txn_id = test_txn(1);

        att.begin(txn_id, test_lsn(100));
        att.commit(txn_id);

        assert!(att.get_snapshot(txn_id).is_none());
    }

    #[test]
    fn test_att_updates_current_lsn() {
        let mut att = ActiveTransactionTable::new();

        att.begin(test_txn(1), test_lsn(100));
        assert_eq!(att.current_lsn, test_lsn(100));

        att.begin(test_txn(2), test_lsn(200));
        assert_eq!(att.current_lsn, test_lsn(200));
    }
}
