//! Persistent storage engine implementation using heap files and catalog.

use crate::mvcc::ActiveTransactionTable;
use crate::storage::StorageManager;
use crate::wal::{TransactionId, TransactionIdGenerator, WalManager, WalRecord, recover};
use relvar_core::storage_engine::{RelationMetadata, StorageEngine, StorageError};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::HashSet;
use std::path::Path;

/// Snapshot for persistent transactions.
///
/// Represents the state of a transaction at a specific point in time.
#[derive(Debug, Clone)]
pub struct PersistentSnapshot {
    /// The transaction ID.
    pub txn_id: TransactionId,
}

/// Persistent storage engine using heap files and JSON catalog.
///
/// This engine persists all data to disk using:
/// - Heap files for tuple storage (one file per relation)
/// - JSON catalog for metadata
///
/// # Example
///
/// ```no_run
/// use relvar_storage::PersistentEngine;
/// use relvar_core::storage_engine::StorageEngine;
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
///
/// let mut engine = PersistentEngine::open("my_db").unwrap();
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
///
/// engine.create_relation("EMPLOYEES", rel_type).unwrap();
/// ```
pub struct PersistentEngine {
    /// Storage Manager handling Catalog and HeapFiles.
    storage: StorageManager,
    /// Write-Ahead Log manager.
    wal: WalManager,
    /// Transaction ID generator.
    txn_id_gen: TransactionIdGenerator,
    /// Active transaction table for MVCC.
    active_txns: ActiveTransactionTable,
    /// Set of committed transaction IDs for visibility checks.
    committed_txns: HashSet<TransactionId>,
    /// Current transaction context for operations.
    current_txn: Option<TransactionId>,
}

impl PersistentEngine {
    /// Open or create a database at the specified path.
    ///
    /// If the database directory does not exist, it will be created along with
    /// an empty catalog. If the database already exists, the catalog is loaded
    /// from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if I/O operations fail.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();
        let wal_path = db_path.join("wal.log");

        // Open Storage Manager (handles directory creation and catalog loading)
        let mut storage = StorageManager::open(db_path.clone())?;

        // Open WAL Manager
        // Note: StorageManager ensures directory exists, so WAL open should be safe
        let mut wal = if wal_path.exists() {
            WalManager::open(&wal_path).map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?
        } else {
            WalManager::create(&wal_path).map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?
        };

        // Perform crash recovery if needed
        let recovery_result =
            recover(&mut wal).map_err(|e| StorageError::Other(format!("Recovery error: {}", e)))?;

        // Seed transaction ID generator with max ID from WAL + 1 to avoid reuse
        let txn_id_gen = TransactionIdGenerator::from_start(TransactionId::new(
            recovery_result.max_txn_id.value() + 1,
        ));

        // Populate committed transactions from recovery
        let committed_txns = recovery_result.committed_txns;

        // Undo uncommitted transactions
        // We pass current LSN from WAL to allow creating a proper snapshot for loading
        let current_lsn = wal.current_lsn();
        storage.undo_uncommitted_inserts_with_committed(
            recovery_result.uncommitted_inserts,
            current_lsn,
            &committed_txns,
        )?;

        Ok(Self {
            storage,
            wal,
            txn_id_gen,
            active_txns: ActiveTransactionTable::new(),
            committed_txns,
            current_txn: None,
        })
    }

    /// Performs a checkpoint to enable WAL truncation and faster recovery.
    ///
    /// A checkpoint:
    /// 1. Flushes all dirty pages to disk
    /// 2. Records the minimum LSN of active transactions
    /// 3. Writes a checkpoint record to the WAL
    /// 4. Allows old WAL records to be truncated
    ///
    /// # Errors
    ///
    /// Returns an error if flushing or WAL operations fail.
    pub fn checkpoint(&mut self) -> Result<(), StorageError> {
        // CRITICAL: Flush WAL first to ensure all prior modifications are logged
        // This upholds the WAL protocol: log must be on disk before data pages
        self.flush_wal()?;

        // Now safe to flush heap files (dirty pages to disk)
        self.storage.flush_all()?;

        // Determine minimum active LSN
        let min_active_lsn = self.get_checkpoint_lsn();

        // Log checkpoint record
        self.log_checkpoint_record(min_active_lsn)?;

        // Flush WAL to ensure checkpoint is durable
        self.flush_wal()?;

        // Garbage collect old versions
        self.garbage_collect_versions()?;

        // TODO: Truncate old WAL records before min_active_lsn
        // This would require WalManager.truncate(lsn) method

        Ok(())
    }

    /// Flush WAL to disk.
    fn flush_wal(&mut self) -> Result<(), StorageError> {
        self.wal
            .flush()
            .map_err(|e| StorageError::Other(format!("WAL flush error: {}", e)))
    }

    /// Get the LSN to checkpoint from (oldest active or current).
    fn get_checkpoint_lsn(&self) -> crate::wal::Lsn {
        self.active_txns
            .oldest_active_lsn()
            .unwrap_or_else(|| self.wal.current_lsn())
    }

    /// Log checkpoint record to WAL.
    fn log_checkpoint_record(
        &mut self,
        min_active_lsn: crate::wal::Lsn,
    ) -> Result<(), StorageError> {
        // Collect dirty pages (simplified - all open heap files are considered dirty)
        let dirty_pages = std::collections::HashMap::new();

        self.wal
            .log(WalRecord::Checkpoint {
                min_active_lsn,
                dirty_pages,
            })
            .map_err(|e| StorageError::Other(format!("WAL checkpoint error: {}", e)))?;
        Ok(())
    }

    /// Garbage collect old versions.
    fn garbage_collect_versions(&mut self) -> Result<(), StorageError> {
        let gc_lsn = self.get_checkpoint_lsn();
        self.storage.garbage_collect_versions(gc_lsn, &self.committed_txns)
    }

    /// Get snapshot for the current transaction context.
    fn get_snapshot_for_current_context(
        &self,
    ) -> Result<crate::mvcc::TransactionSnapshot, StorageError> {
        if let Some(txn_id) = self.current_txn {
            // In transaction - use snapshot isolation
            self.active_txns
                .get_snapshot(txn_id)
                .cloned()
                .ok_or_else(|| {
                    StorageError::Other(format!("Transaction {} not found", txn_id.value()))
                })
        } else {
            // Outside transaction - see all committed data
            // Create ad-hoc snapshot with no active transactions
            Ok(crate::mvcc::TransactionSnapshot::new(
                TransactionId::new(0),
                self.wal.current_lsn(),
                vec![],
            ))
        }
    }
}

impl StorageEngine for PersistentEngine {
    type Snapshot = PersistentSnapshot;

    fn create_relation(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), StorageError> {
        self.storage.create_relation(name, relation_type)
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        self.storage.drop_relation(name)
    }

    fn relation_exists(&self, name: &str) -> bool {
        self.storage.relation_exists(name)
    }

    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        self.storage.get_relation_metadata(name)
    }

    fn list_relations(&self) -> Vec<String> {
        self.storage.list_relations()
    }

    fn load_relation(&self, name: &str) -> Result<Relation, StorageError> {
        // Use MVCC visibility if in a transaction, otherwise see all committed data
        let snapshot = self.get_snapshot_for_current_context()?;

        // Use StorageManager's read-only load_relation (bypasses cache)
        self.storage.load_relation(name, &snapshot, &self.committed_txns)
    }

    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        // Auto-commit transaction if not in explicit transaction
        let auto_commit = self.current_txn.is_none();
        let txn_id = if auto_commit {
            let snapshot = self.begin_transaction()?;
            snapshot.txn_id
        } else {
            self.current_txn.unwrap()
        };

        // Delegate to StorageManager
        self.storage.store_relation(name, relation, txn_id)?;

        // Auto-commit if needed
        if auto_commit {
            let snapshot = PersistentSnapshot { txn_id };
            self.commit_transaction(snapshot)?;
        }

        Ok(())
    }

    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        // Auto-commit if not in explicit transaction
        let auto_commit = self.current_txn.is_none();

        let txn_id = if auto_commit {
            // Start auto-commit transaction
            let snapshot = self.begin_transaction()?;
            snapshot.txn_id
        } else {
            self.current_txn.unwrap()
        };

        // Serialize tuple for WAL
        let tuple_data = bincode::serialize(&tuple)
            .map_err(|e| StorageError::Other(format!("Tuple serialization error: {}", e)))?;

        // Log INSERT record to WAL (before data modification)
        self.wal
            .log(WalRecord::Insert {
                txn_id,
                relation_name: name.to_string(),
                tuple_data,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        // Insert into heap file with MVCC versioning via StorageManager
        self.storage.insert_tuple_versioned(name, &tuple, txn_id)?;

        // Auto-commit if needed
        if auto_commit {
            let snapshot = PersistentSnapshot { txn_id };
            self.commit_transaction(snapshot)?;
        }

        Ok(())
    }

    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError> {
        // Generate transaction ID
        let txn_id = self.txn_id_gen.generate();

        // Get current LSN from WAL
        let current_lsn = self.wal.current_lsn();

        // Log BEGIN record to WAL
        self.wal
            .log(WalRecord::Begin { txn_id })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        // Add to active transaction table (creates snapshot)
        let _snapshot = self.active_txns.begin(txn_id, current_lsn);

        // Set as current transaction context
        self.current_txn = Some(txn_id);

        // MVCC handles rollback via visibility - no need to save relations
        Ok(PersistentSnapshot { txn_id })
    }

    fn commit_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        // Log COMMIT record to WAL
        self.wal
            .log(WalRecord::Commit {
                txn_id: snapshot.txn_id,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        // Flush WAL to ensure durability
        self.wal
            .flush()
            .map_err(|e| StorageError::Other(format!("WAL flush error: {}", e)))?;

        // Flush all open heap files to disk
        self.storage.flush_all()?;

        // Add to committed transactions set
        self.committed_txns.insert(snapshot.txn_id);

        // Remove from active transactions
        self.active_txns.commit(snapshot.txn_id);

        // Clear current transaction context
        self.current_txn = None;

        Ok(())
    }

    fn rollback_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        // Log ABORT record to WAL
        self.wal
            .log(WalRecord::Abort {
                txn_id: snapshot.txn_id,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        // Do NOT flush WAL - aborted transactions are not durable

        // MVCC rollback: Uncommitted versions are automatically invisible
        // No physical data restoration needed - visibility rules handle it

        // Remove from active transactions
        self.active_txns.abort(snapshot.txn_id);

        // Clear current transaction context
        self.current_txn = None;

        Ok(())
    }
}

// MVCC-specific methods (internal, not part of StorageEngine trait)
impl PersistentEngine {
    /// Loads a relation with MVCC visibility filtering.
    ///
    /// Only returns tuples visible to the given transaction according to
    /// MVCC snapshot isolation rules.
    ///
    /// # Arguments
    /// * `name` - Relation name
    /// * `txn_id` - Transaction ID to determine visibility
    ///
    /// # Errors
    /// Returns error if relation doesn't exist or visibility check fails.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn load_relation_for_txn(
        &mut self,
        name: &str,
        txn_id: TransactionId,
    ) -> Result<Relation, StorageError> {
        // Get transaction snapshot
        let snapshot = self
            .active_txns
            .get_snapshot(txn_id)
            .ok_or_else(|| {
                StorageError::Other(format!("Transaction {} not found", txn_id.value()))
            })?
            .clone();

        // Use StorageManager's read-only load_relation (bypasses cache)
        self.storage.load_relation(name, &snapshot, &self.committed_txns)
    }

    /// Inserts a tuple with MVCC version tracking.
    ///
    /// # Arguments
    /// * `name` - Relation name
    /// * `tuple` - Tuple to insert
    /// * `txn_id` - Transaction ID creating this version
    ///
    /// # Errors
    /// Returns error if relation doesn't exist or insert fails.
    ///
    /// NOTE: Currently unused - reserved for future MVCC transaction implementation.
    #[allow(dead_code)]
    pub(crate) fn insert_tuple_in_txn(
        &mut self,
        name: &str,
        tuple: Tuple,
        txn_id: TransactionId,
    ) -> Result<(), StorageError> {
        // Serialize tuple for WAL
        let tuple_data = bincode::serialize(&tuple)
            .map_err(|e| StorageError::Other(format!("Tuple serialization error: {}", e)))?;

        // Log INSERT record to WAL (before data modification)
        self.wal
            .log(WalRecord::Insert {
                txn_id,
                relation_name: name.to_string(),
                tuple_data,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        // Insert tuple with version metadata via StorageManager
        self.storage.insert_tuple_versioned(name, &tuple, txn_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{ScalarType, TupleType};
    use tempfile::TempDir;

    fn test_rel_type() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        )
    }

    #[test]
    fn test_create_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();
            engine.insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" }).unwrap();
        }
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            assert!(engine.relation_exists("TEST"));
            let relation = engine.load_relation("TEST").unwrap();
            assert_eq!(relation.cardinality(), 1);
        }
    }

    #[test]
    fn test_transaction_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine.insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" }).unwrap();
        let snapshot = engine.begin_transaction().unwrap();
        engine.insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 2);
        engine.rollback_transaction(snapshot).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_recovery_undoes_uncommitted_data() {
        let temp_dir = TempDir::new().unwrap();
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();
            let _snapshot = engine.begin_transaction().unwrap();
            engine.insert_tuple("TEST", tuple! { id: 1i64, name: "Uncommitted" }).unwrap();
        }
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let relation = engine.load_relation("TEST").unwrap();
            assert_eq!(relation.cardinality(), 0);
        }
    }
}
