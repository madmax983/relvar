use crate::mvcc::ActiveTransactionTable;
use crate::wal::{Lsn, TransactionId, TransactionIdGenerator};
use relvar_core::values::Relation;
use std::collections::{HashMap, HashSet};

/// Snapshot for persistent transactions.
#[derive(Debug, Clone)]
pub struct PersistentSnapshot {
    /// The transaction ID.
    pub txn_id: TransactionId,
    /// Saved relations (for compatibility with old rollback approach).
    /// TODO: Remove once full WAL recovery is implemented.
    pub saved_relations: HashMap<String, Relation>,
}

/// Manages transaction state and lifecycle.
pub struct TransactionManager {
    /// Transaction ID generator.
    txn_id_gen: TransactionIdGenerator,
    /// Active transaction table for MVCC.
    active_txns: ActiveTransactionTable,
    /// Set of committed transaction IDs for visibility checks.
    committed_txns: HashSet<TransactionId>,
    /// Current transaction context for operations.
    current_txn: Option<TransactionId>,
}

impl TransactionManager {
    /// Create a new transaction manager.
    pub fn new(txn_id_gen: TransactionIdGenerator, committed_txns: HashSet<TransactionId>) -> Self {
        Self {
            txn_id_gen,
            active_txns: ActiveTransactionTable::new(),
            committed_txns,
            current_txn: None,
        }
    }

    /// Generate a new unique transaction ID.
    pub fn generate_txn_id(&mut self) -> TransactionId {
        self.txn_id_gen.generate()
    }

    /// Register a new transaction as active.
    pub fn begin(&mut self, txn_id: TransactionId, current_lsn: Lsn) -> PersistentSnapshot {
        // Add to active transaction table
        self.active_txns.begin(txn_id, current_lsn);

        // Set as current transaction context
        self.current_txn = Some(txn_id);

        PersistentSnapshot {
            txn_id,
            saved_relations: HashMap::new(),
        }
    }

    /// Mark a transaction as committed.
    pub fn commit(&mut self, txn_id: TransactionId) {
        // Add to committed transactions set
        self.committed_txns.insert(txn_id);

        // Remove from active transactions
        self.active_txns.commit(txn_id);

        // Clear current transaction context if it matches
        if self.current_txn == Some(txn_id) {
            self.current_txn = None;
        }
    }

    /// Mark a transaction as aborted/rolled back.
    pub fn rollback(&mut self, txn_id: TransactionId) {
        // Remove from active transactions
        self.active_txns.abort(txn_id);

        // Clear current transaction context if it matches
        if self.current_txn == Some(txn_id) {
            self.current_txn = None;
        }
    }

    /// Get the ID of the current transaction, if any.
    pub fn current_txn(&self) -> Option<TransactionId> {
        self.current_txn
    }

    /// Set the current transaction context.
    pub fn set_current_txn(&mut self, txn_id: Option<TransactionId>) {
        self.current_txn = txn_id;
    }

    /// Get reference to active transaction table.
    pub fn active_txns(&self) -> &ActiveTransactionTable {
        &self.active_txns
    }

    /// Get reference to committed transactions set.
    pub fn committed_txns(&self) -> &HashSet<TransactionId> {
        &self.committed_txns
    }
}
