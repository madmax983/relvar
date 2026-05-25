//! Persistent storage engine implementation using heap files and catalog.

use crate::mvcc::ActiveTransactionTable;
use crate::storage::StorageManager;
use crate::wal::{TransactionId, TransactionIdGenerator, WalManager, WalRecord, recover};
use relvar_core::storage_engine::{RelationMetadata, StorageEngine, StorageError};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::HashSet;
use std::path::Path;
use std::sync::RwLock;

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
/// # Persistence & Concurrency
///
/// The engine implements ACID properties using standard techniques:
///
/// - **Durability (WAL):** A Write-Ahead Log ensures that all changes are recorded
///   before being applied to the data files. In the event of a crash, the engine
///   replays committed transactions and undoes uncommitted ones during recovery.
/// - **Isolation (MVCC):** Multi-Version Concurrency Control allows multiple
///   transactions to read and write simultaneously without locking. Each transaction
///   sees a consistent snapshot of the database as of its start time. Writes create
///   new versions of tuples rather than overwriting them in place.
///
/// # Examples
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
    /// Storage Manager for physical data handling.
    storage_manager: RwLock<StorageManager>,
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
    /// Yields an error if I/O operations fail.
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::PersistentEngine;
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let opened = PersistentEngine::open(dir.path()).unwrap();
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();
        let wal_path = db_path.join("wal.log");

        // Initialize StorageManager (handles DB dir and catalog)
        let storage_manager = StorageManager::new(&db_path)?;

        let mut wal = Self::open_wal_manager(&wal_path)?;

        // Perform crash recovery if needed
        let recovery_result =
            recover(&mut wal).map_err(|e| StorageError::Other(format!("Recovery error: {}", e)))?;

        // Create engine instance
        let mut engine = Self {
            storage_manager: RwLock::new(storage_manager),
            wal,
            // Seed transaction ID generator with max ID from WAL + 1 to avoid reuse
            txn_id_gen: TransactionIdGenerator::from_start(TransactionId::new(
                recovery_result.max_txn_id.value() + 1,
            )),
            active_txns: ActiveTransactionTable::new(),
            committed_txns: recovery_result.committed_txns, // Populate from recovery
            current_txn: None,
        };

        // Undo uncommitted transactions
        engine.undo_uncommitted_inserts(recovery_result.uncommitted_inserts)?;

        Ok(engine)
    }

    /// Undoes uncommitted inserts identified during recovery.
    fn undo_uncommitted_inserts(
        &mut self,
        uncommitted_inserts: Vec<crate::wal::UncommittedInsert>,
    ) -> Result<(), StorageError> {
        let relations_to_cleanup = self.group_uncommitted_inserts(uncommitted_inserts);

        // For each relation, rebuild without uncommitted tuples
        for relation_name in relations_to_cleanup {
            if self
                .storage_manager
                .read()
                .unwrap()
                .relation_exists(&relation_name)
            {
                self.cleanup_relation_uncommitted_inserts(&relation_name)?;
            }
        }

        Ok(())
    }

    /// Groups uncommitted inserts by relation name.
    fn group_uncommitted_inserts(
        &self,
        uncommitted_inserts: Vec<crate::wal::UncommittedInsert>,
    ) -> std::collections::HashSet<String> {
        uncommitted_inserts
            .into_iter()
            .map(|insert| insert.relation_name)
            .collect()
    }

    /// Rebuilds a relation excluding uncommitted tuples and stores it back.
    fn cleanup_relation_uncommitted_inserts(
        &mut self,
        relation_name: &str,
    ) -> Result<(), StorageError> {
        // Load all tuples using current snapshot (txn=0, committed_txns set from recovery)
        let snapshot = self.get_snapshot_for_current_context()?;

        // scan_relation implicitly filters out uncommitted tuples because they are not in committed_txns
        let relation = self
            .storage_manager
            .write()
            .map_err(|_| StorageError::Other("Storage manager lock poisoned".to_string()))?
            .scan_relation(relation_name, &snapshot, &self.committed_txns)?;

        // Store back (effectively removing uncommitted garbage from the heap file)
        self.store_relation(relation_name, &relation)?;
        Ok(())
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
    /// Yields an error if flushing or WAL operations fail.
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::PersistentEngine;
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let mut engine = PersistentEngine::open(dir.path()).unwrap();
    /// engine.checkpoint().unwrap();
    /// ```
    pub fn checkpoint(&mut self) -> Result<(), StorageError> {
        // CRITICAL: Flush WAL first to ensure all prior modifications are logged
        self.flush_wal()?;

        // Now safe to flush heap files (dirty pages to disk)
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("Storage manager lock poisoned".to_string()))?
            .flush_heap_files()?;

        // Determine minimum active LSN
        let min_active_lsn = self.get_checkpoint_lsn();

        // Log checkpoint record
        self.log_checkpoint_record(min_active_lsn)?;

        // Flush WAL to ensure checkpoint is durable
        self.flush_wal()?;

        // Garbage collect old versions
        let gc_lsn = self.get_checkpoint_lsn();
        self.storage_manager
            .write()
            .unwrap()
            .garbage_collect_versions(gc_lsn, &self.committed_txns)?;

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
        // Collect dirty pages logic is simplified - assume all managed files are potentially dirty
        // A real implementation would track dirty pages in StorageManager
        let dirty_pages = std::collections::HashMap::new();

        self.wal
            .log(WalRecord::Checkpoint {
                min_active_lsn,
                dirty_pages,
            })
            .map_err(|e| StorageError::Other(format!("WAL checkpoint error: {}", e)))?;
        Ok(())
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
            Ok(crate::mvcc::TransactionSnapshot::new(
                TransactionId::new(0),
                self.wal.current_lsn(),
                vec![],
            ))
        }
    }

    /// Open or create WAL manager.
    fn open_wal_manager(wal_path: &Path) -> Result<WalManager, StorageError> {
        if wal_path.exists() {
            WalManager::open(wal_path).map_err(|e| StorageError::Other(format!("WAL error: {}", e)))
        } else {
            WalManager::create(wal_path)
                .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))
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
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("Storage manager lock poisoned".to_string()))?
            .create_relation(name, relation_type)
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("Storage manager lock poisoned".to_string()))?
            .drop_relation(name)
    }

    fn relation_exists(&self, name: &str) -> bool {
        self.storage_manager
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .relation_exists(name)
    }

    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        self.storage_manager
            .read()
            .map_err(|_| StorageError::Other("Storage manager lock poisoned".to_string()))?
            .get_relation_metadata(name)
    }

    fn list_relations(&self) -> Vec<String> {
        self.storage_manager
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .list_relations()
    }

    fn load_relation(&self, name: &str) -> Result<Relation, StorageError> {
        // Use MVCC visibility if in a transaction, otherwise see all committed data
        let snapshot = self.get_snapshot_for_current_context()?;
        self.storage_manager
            .write()
            .unwrap()
            .scan_relation(name, &snapshot, &self.committed_txns)
    }

    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        let (txn_id, auto_commit) = self.ensure_transaction()?;

        self.storage_manager
            .write()
            .unwrap()
            .store_relation(name, relation, txn_id)?;

        if auto_commit {
            let snapshot = PersistentSnapshot { txn_id };
            self.commit_transaction(snapshot)?;
        }

        Ok(())
    }

    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        let (txn_id, auto_commit) = self.ensure_transaction()?;

        // Serialize tuple for WAL
        let tuple_data = postcard::to_allocvec(&tuple)
            .map_err(|e| StorageError::Other(format!("Tuple serialization error: {}", e)))?;

        // Log INSERT record to WAL
        self.wal
            .log(WalRecord::Insert {
                txn_id,
                relation_name: name.to_string(),
                tuple_data,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        self.storage_manager
            .write()
            .unwrap()
            .insert_tuple(name, tuple, txn_id)?;

        if auto_commit {
            let snapshot = PersistentSnapshot { txn_id };
            self.commit_transaction(snapshot)?;
        }

        Ok(())
    }

    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError> {
        let txn_id = self.txn_id_gen.generate();
        let current_lsn = self.wal.current_lsn();

        self.wal
            .log(WalRecord::Begin { txn_id })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        let _snapshot = self.active_txns.begin(txn_id, current_lsn);
        self.current_txn = Some(txn_id);

        Ok(PersistentSnapshot { txn_id })
    }

    fn commit_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        self.wal
            .log(WalRecord::Commit {
                txn_id: snapshot.txn_id,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        self.wal
            .flush()
            .map_err(|e| StorageError::Other(format!("WAL flush error: {}", e)))?;

        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("Storage manager lock poisoned".to_string()))?
            .flush_heap_files()?;

        self.committed_txns.insert(snapshot.txn_id);
        self.active_txns.commit(snapshot.txn_id);
        self.current_txn = None;

        Ok(())
    }

    fn rollback_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        self.wal
            .log(WalRecord::Abort {
                txn_id: snapshot.txn_id,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        self.active_txns.abort(snapshot.txn_id);
        self.current_txn = None;

        Ok(())
    }
}

// MVCC methods (internal)
impl PersistentEngine {
    /// Ensures a transaction is active.
    ///
    /// Provides the current transaction ID along with an auto-commit status flag.
    fn ensure_transaction(&mut self) -> Result<(TransactionId, bool), StorageError> {
        if let Some(txn_id) = self.current_txn {
            Ok((txn_id, false))
        } else {
            let snapshot = self.begin_transaction()?;
            Ok((snapshot.txn_id, true))
        }
    }

    #[allow(dead_code)]
    pub(crate) fn load_relation_for_txn(
        &mut self,
        name: &str,
        txn_id: TransactionId,
    ) -> Result<Relation, StorageError> {
        let snapshot = self
            .active_txns
            .get_snapshot(txn_id)
            .ok_or_else(|| {
                StorageError::Other(format!("Transaction {} not found", txn_id.value()))
            })?
            .clone();

        self.storage_manager
            .write()
            .unwrap()
            .scan_relation(name, &snapshot, &self.committed_txns)
    }

    #[allow(dead_code)]
    pub(crate) fn insert_tuple_in_txn(
        &mut self,
        name: &str,
        tuple: Tuple,
        txn_id: TransactionId,
    ) -> Result<(), StorageError> {
        // Log WAL
        let tuple_data = postcard::to_allocvec(&tuple)
            .map_err(|e| StorageError::Other(format!("Tuple serialization error: {}", e)))?;

        self.wal
            .log(WalRecord::Insert {
                txn_id,
                relation_name: name.to_string(),
                tuple_data,
            })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        self.storage_manager
            .write()
            .unwrap()
            .insert_tuple(name, tuple, txn_id)
    }
}

#[cfg(test)]
mod tests;
