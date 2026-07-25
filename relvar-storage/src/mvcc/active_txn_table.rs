//! # Active Transaction Table (ATT)
//!
//! Tracks concurrent, uncommitted transactions to provide snapshot isolation.
//!
//! The ATT is the source of truth for which transactions are currently running
//! in the database engine. By capturing a snapshot of the ATT at the moment a
//! new transaction begins, the database knows exactly which concurrent changes
//! must be hidden from the new transaction's view (Snapshot Isolation).

use crate::mvcc::TransactionSnapshot;
use crate::wal::{Lsn, TransactionId};
use std::collections::HashMap;

/// Tracks all active (uncommitted) transactions.
///
/// This structure is central to Snapshot Isolation. It maintains the set of all
/// transactions that have begun but not yet committed or aborted. When a new
/// transaction starts, it copies the keys of this table into its [`TransactionSnapshot`].
///
/// ## Examples
///
/// ```ignore
/// use relvar_storage::mvcc::ActiveTransactionTable;
/// use relvar_storage::wal::{Lsn, TransactionId};
///
/// let mut att = ActiveTransactionTable::new();
///
/// // Start transaction 1
/// let t1 = TransactionId::new(1);
/// let snapshot1 = att.begin(t1, Lsn::new(100));
/// assert!(!snapshot1.is_active(t1)); // Not active when snapshot was taken
///
/// // Start transaction 2 concurrently
/// let t2 = TransactionId::new(2);
/// let snapshot2 = att.begin(t2, Lsn::new(150));
/// assert!(snapshot2.is_active(t1)); // T2 sees T1 as an active, uncommitted transaction
///
/// // T1 finishes
/// att.commit(t1);
///
/// // Start transaction 3
/// let t3 = TransactionId::new(3);
/// let snapshot3 = att.begin(t3, Lsn::new(200));
/// assert!(!snapshot3.is_active(t1)); // T1 committed, so it's no longer active
/// assert!(snapshot3.is_active(t2));  // T2 is still running
/// ```ignore
#[derive(Debug)]
pub(crate) struct ActiveTransactionTable {
    transactions: HashMap<TransactionId, TransactionSnapshot>,
    current_lsn: Lsn,
}

impl ActiveTransactionTable {
    /// Creates a new empty transaction table.
    pub(crate) fn new() -> Self {
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
    /// Begins a new transaction and generates its snapshot.
    ///
    /// This method performs two critical operations atomically:
    /// 1. Takes a "picture" of all currently running transactions to form a [`TransactionSnapshot`].
    /// 2. Registers the new `txn_id` as an active transaction so future transactions will see it.
    ///
    /// # Arguments
    /// * `txn_id` - The unique identifier for the new transaction.
    /// * `lsn` - The Current Log Sequence Number (LSN) marking the point in time this transaction began.
    ///
    /// # Returns
    /// A [`TransactionSnapshot`] that the transaction will carry for its entire lifetime
    /// to determine tuple visibility.
    ///
    /// ## Examples
    ///
    /// ```ignore
    /// use relvar_storage::mvcc::ActiveTransactionTable;
    /// use relvar_storage::wal::{Lsn, TransactionId};
    ///
    /// let mut att = ActiveTransactionTable::new();
    /// let snapshot = att.begin(TransactionId::new(42), Lsn::new(100));
    ///
    /// assert_eq!(snapshot.txn_id, TransactionId::new(42));
    /// assert_eq!(snapshot.snapshot_lsn, Lsn::new(100));
    /// ```ignore
    pub(crate) fn begin(&mut self, txn_id: TransactionId, lsn: Lsn) -> TransactionSnapshot {
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

    /// Commits a transaction, removing it from the active set.
    ///
    /// Once removed, any *new* transactions that begin will no longer see this `txn_id`
    /// in their snapshot, meaning this transaction's changes will be visible to them.
    ///
    /// # Arguments
    /// * `txn_id` - The transaction that is committing.
    ///
    /// ## Examples
    ///
    /// ```ignore
    /// use relvar_storage::mvcc::ActiveTransactionTable;
    /// use relvar_storage::wal::{Lsn, TransactionId};
    ///
    /// let mut att = ActiveTransactionTable::new();
    /// let t1 = TransactionId::new(1);
    ///
    /// att.begin(t1, Lsn::new(100));
    /// // T1 does some work...
    /// att.commit(t1); // T1's changes are now universally visible to new txns
    /// ```ignore
    pub(crate) fn commit(&mut self, txn_id: TransactionId) {
        self.transactions.remove(&txn_id);
    }

    /// Aborts a transaction, removing it from the active set.
    ///
    /// Similar to `commit`, this removes the transaction from the ATT. However,
    /// the transaction's changes will remain invisible to future transactions
    /// because aborted transactions never enter the "Committed Transactions" list
    /// maintained elsewhere in the engine.
    ///
    /// # Arguments
    /// * `txn_id` - The transaction that is rolling back.
    pub(crate) fn abort(&mut self, txn_id: TransactionId) {
        self.transactions.remove(&txn_id);
    }

    /// Retrieves the oldest LSN from the pool of currently active transactions.
    ///
    /// This is a critical method for the Vacuum/Garbage Collection (GC) subsystem.
    /// Any tuple version that was deleted *before* this `oldest_active_lsn` is
    /// guaranteed to be invisible to *all* currently running and future transactions.
    /// Therefore, the GC engine can safely physically delete those tuple versions from disk.
    ///
    /// # Returns
    /// * `Some(Lsn)` - The lowest `snapshot_lsn` among all active transactions.
    /// * `None` - If no transactions are currently active.
    ///
    /// ## Examples
    ///
    /// ```ignore
    /// use relvar_storage::mvcc::ActiveTransactionTable;
    /// use relvar_storage::wal::{Lsn, TransactionId};
    ///
    /// let mut att = ActiveTransactionTable::new();
    /// assert_eq!(att.oldest_active_lsn(), None);
    ///
    /// att.begin(TransactionId::new(1), Lsn::new(100));
    /// att.begin(TransactionId::new(2), Lsn::new(200));
    ///
    /// // The oldest transaction started at LSN 100
    /// assert_eq!(att.oldest_active_lsn(), Some(Lsn::new(100)));
    /// ```ignore
    pub(crate) fn oldest_active_lsn(&self) -> Option<Lsn> {
        self.transactions
            .values()
            .map(|snapshot| snapshot.snapshot_lsn)
            .min()
    }

    /// Retrieves the original snapshot generated for an active transaction.
    ///
    /// # Arguments
    /// * `txn_id` - The transaction ID to lookup.
    ///
    /// # Returns
    /// * `Some(&TransactionSnapshot)` - The snapshot if the transaction is currently active.
    /// * `None` - If the transaction has committed, aborted, or never existed.
    pub(crate) fn get_snapshot(&self, txn_id: TransactionId) -> Option<&TransactionSnapshot> {
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
