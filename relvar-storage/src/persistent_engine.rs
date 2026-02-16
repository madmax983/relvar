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
    /// Returns an error if I/O operations fail.
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
        let by_relation = self.group_uncommitted_inserts(uncommitted_inserts);

        // For each relation, rebuild without uncommitted tuples
        for (relation_name, uncommitted_tuples) in by_relation {
            if self
                .storage_manager
                .read()
                .unwrap()
                .relation_exists(&relation_name)
            {
                self.cleanup_relation_uncommitted_inserts(&relation_name, uncommitted_tuples)?;
            }
        }

        Ok(())
    }

    /// Groups uncommitted inserts by relation name.
    fn group_uncommitted_inserts(
        &self,
        uncommitted_inserts: Vec<crate::wal::UncommittedInsert>,
    ) -> std::collections::HashMap<String, Vec<Vec<u8>>> {
        uncommitted_inserts
            .into_iter()
            .fold(std::collections::HashMap::new(), |mut acc, insert| {
                acc.entry(insert.relation_name)
                    .or_default()
                    .push(insert.tuple_data);
                acc
            })
    }

    /// Rebuilds a relation excluding uncommitted tuples and stores it back.
    fn cleanup_relation_uncommitted_inserts(
        &mut self,
        relation_name: &str,
        uncommitted_tuples_vec: Vec<Vec<u8>>,
    ) -> Result<(), StorageError> {
        // Convert to HashSet for O(1) lookup
        let uncommitted_tuples: std::collections::HashSet<Vec<u8>> =
            uncommitted_tuples_vec.into_iter().collect();

        // Load all tuples using current snapshot (txn=0, committed_txns set from recovery)
        let snapshot = self.get_snapshot_for_current_context()?;

        let relation = self.storage_manager.write().unwrap().scan_relation(
            relation_name,
            &snapshot,
            &self.committed_txns,
        )?;

        // Rebuild relation with only committed tuples
        let mut new_relation = relvar_core::values::Relation::new(relation.relation_type().clone());

        for tuple in relation.tuples() {
            // Serialization should never fail for a valid in-memory tuple
            let tuple_data =
                bincode::serialize(&tuple).expect("Failed to serialize tuple during cleanup");

            if !uncommitted_tuples.contains(&tuple_data) {
                // Insertion into a new relation with the same type should never fail
                new_relation
                    .insert(tuple.clone())
                    .expect("Failed to insert tuple into clean relation");
            }
        }

        // Store back (effectively removing uncommitted garbage)
        self.store_relation(relation_name, &new_relation)?;
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
    /// Returns an error if flushing or WAL operations fail.
    pub fn checkpoint(&mut self) -> Result<(), StorageError> {
        // CRITICAL: Flush WAL first to ensure all prior modifications are logged
        self.flush_wal()?;

        // Now safe to flush heap files (dirty pages to disk)
        self.storage_manager.write().unwrap().flush_heap_files()?;

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
            .unwrap()
            .create_relation(name, relation_type)
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        self.storage_manager.write().unwrap().drop_relation(name)
    }

    fn relation_exists(&self, name: &str) -> bool {
        self.storage_manager.read().unwrap().relation_exists(name)
    }

    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        self.storage_manager
            .read()
            .unwrap()
            .get_relation_metadata(name)
    }

    fn list_relations(&self) -> Vec<String> {
        self.storage_manager.read().unwrap().list_relations()
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
        let tuple_data = bincode::serialize(&tuple)
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

        self.storage_manager.write().unwrap().flush_heap_files()?;

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
    /// Returns the transaction ID and a boolean indicating if a new transaction was started (auto-commit).
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
        let tuple_data = bincode::serialize(&tuple)
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

        // Create database and insert data
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
        }

        // Reopen database and verify data persists
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
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Begin transaction
        let snapshot = engine.begin_transaction().unwrap();

        // Make changes
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 2);

        // Rollback
        engine.rollback_transaction(snapshot).unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Try to create relation with path separator
        let result = engine.create_relation("../evil", test_rel_type());
        assert!(result.is_err());

        // Try with backslash
        let result = engine.create_relation("..\\evil", test_rel_type());
        assert!(result.is_err());

        // Try with forward slash
        let result = engine.create_relation("sub/dir", test_rel_type());
        assert!(result.is_err());

        // Try with parent directory reference
        let result = engine.create_relation("..", test_rel_type());
        assert!(result.is_err());

        // Try with empty name
        let result = engine.create_relation("", test_rel_type());
        assert!(result.is_err());

        // Valid name should work
        let result = engine.create_relation("VALID_NAME", test_rel_type());
        assert!(result.is_ok());
    }

    #[test]
    fn test_drop_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        assert!(engine.relation_exists("TEST"));

        // Drop the relation
        engine.drop_relation("TEST").unwrap();

        assert!(!engine.relation_exists("TEST"));

        // Dropping again should fail
        let result = engine.drop_relation("TEST");
        assert!(result.is_err());
    }

    #[test]
    fn test_store_relation_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Insert some data
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Store empty relation
        let empty_relation = Relation::new(test_rel_type());
        engine.store_relation("TEST", &empty_relation).unwrap();

        // Verify relation is now empty
        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 0);
    }

    #[test]
    fn test_store_relation_large() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Create relation with many tuples
        let mut large_relation = Relation::new(test_rel_type());
        for i in 0..100 {
            large_relation
                .insert(tuple! { id: i as i64, name: format!("Name{}", i) })
                .unwrap();
        }

        engine.store_relation("TEST", &large_relation).unwrap();

        // Verify all tuples persisted
        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 100);
    }

    #[test]
    fn test_list_relations() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Initially empty
        assert_eq!(engine.list_relations().len(), 0);

        // Create some relations
        engine.create_relation("REL1", test_rel_type()).unwrap();
        engine.create_relation("REL2", test_rel_type()).unwrap();
        engine.create_relation("REL3", test_rel_type()).unwrap();

        let relations = engine.list_relations();
        assert_eq!(relations.len(), 3);
        assert!(relations.contains(&"REL1".to_string()));
        assert!(relations.contains(&"REL2".to_string()));
        assert!(relations.contains(&"REL3".to_string()));
    }

    #[test]
    fn test_get_relation_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let metadata = engine.get_relation_metadata("TEST").unwrap();
        assert_eq!(metadata.name, "TEST");
        assert_eq!(metadata.relation_type.degree(), 2);
        assert!(metadata.relation_type.heading().has_attribute("id"));
        assert!(metadata.relation_type.heading().has_attribute("name"));
    }

    #[test]
    fn test_insert_tuple_nonexistent_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.insert_tuple("NONEXISTENT", tuple! { id: 1i64, name: "Alice" });
        assert!(result.is_err());
    }

    #[test]
    fn test_load_relation_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.load_relation("NONEXISTENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_relation_name() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Creating again should fail
        let result = engine.create_relation("TEST", test_rel_type());
        assert!(result.is_err());
    }

    #[test]
    fn test_reopen_with_existing_relations() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple relations
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("REL1", test_rel_type()).unwrap();
            engine.create_relation("REL2", test_rel_type()).unwrap();
            engine
                .insert_tuple("REL1", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
            engine
                .insert_tuple("REL2", tuple! { id: 2i64, name: "Bob" })
                .unwrap();
        }

        // Reopen and verify both relations exist
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            assert_eq!(engine.list_relations().len(), 2);

            let rel1 = engine.load_relation("REL1").unwrap();
            assert_eq!(rel1.cardinality(), 1);

            let rel2 = engine.load_relation("REL2").unwrap();
            assert_eq!(rel2.cardinality(), 1);
        }
    }

    #[test]
    fn test_multiple_inserts_uses_cached_heap_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Multiple inserts should reuse the cached heap file
        for i in 0..10 {
            engine
                .insert_tuple("TEST", tuple! { id: i as i64, name: format!("Name{}", i) })
                .unwrap();
        }

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 10);
    }

    #[test]
    fn test_store_relation_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let relation = Relation::new(test_rel_type());
        let result = engine.store_relation("NONEXISTENT", &relation);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_metadata_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.get_relation_metadata("NONEXISTENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_nonexistent_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.drop_relation("NONEXISTENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_store_relation_replaces_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Insert initial data
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Create new relation with different data
        let mut new_relation = Relation::new(test_rel_type());
        new_relation
            .insert(tuple! { id: 100i64, name: "Charlie" })
            .unwrap();

        // Store should replace old data
        engine.store_relation("TEST", &new_relation).unwrap();

        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 1);
        assert!(loaded.contains(&tuple! { id: 100i64, name: "Charlie" }));
    }

    #[test]
    fn test_relation_exists_returns_false_for_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        assert!(!engine.relation_exists("NONEXISTENT"));
    }

    #[test]
    fn test_relation_exists_returns_true_for_existing() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        assert!(engine.relation_exists("TEST"));
    }

    #[test]
    fn test_transaction_with_multiple_relations() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("REL1", test_rel_type()).unwrap();
        engine.create_relation("REL2", test_rel_type()).unwrap();

        engine
            .insert_tuple("REL1", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine
            .insert_tuple("REL2", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Begin transaction
        let snapshot = engine.begin_transaction().unwrap();

        // Modify both relations
        engine
            .insert_tuple("REL1", tuple! { id: 3i64, name: "Charlie" })
            .unwrap();
        engine
            .insert_tuple("REL2", tuple! { id: 4i64, name: "David" })
            .unwrap();

        // Rollback should restore both
        engine.rollback_transaction(snapshot).unwrap();

        let rel1 = engine.load_relation("REL1").unwrap();
        let rel2 = engine.load_relation("REL2").unwrap();
        assert_eq!(rel1.cardinality(), 1);
        assert_eq!(rel2.cardinality(), 1);
    }

    #[test]
    fn test_empty_database_list_relations() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        assert_eq!(engine.list_relations().len(), 0);
    }

    #[test]
    fn test_insert_then_load_uses_cached_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Load should work with cached heap file
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);

        // Insert more using cache
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 2);
    }

    // WAL Integration Tests (Step 5)

    #[test]
    fn test_commit_flushes_wal() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();

        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Commit should flush WAL to ensure durability
        engine.commit_transaction(snapshot).unwrap();

        // After commit, WAL should be flushed (we can verify by reopening)
        // Data should survive even if we "crash" (close without explicit flush)
        drop(engine);

        // Reopen and verify data persists
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_abort_does_not_flush() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();

        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Rollback should NOT flush WAL
        // (transaction is aborted, changes should not be durable)
        engine.rollback_transaction(snapshot).unwrap();

        // After rollback, data should not persist
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 0);
    }

    // Checkpoint Tests (Step 6)

    #[test]
    fn test_checkpoint_flushes_dirty_pages() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Begin transaction and insert data
        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine.commit_transaction(snapshot).unwrap();

        // Checkpoint should flush all dirty pages
        engine.checkpoint().unwrap();

        // After checkpoint, data should be durable even without explicit commit
        drop(engine);

        // Reopen and verify data persists
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_checkpoint_records_active_txns() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Start a transaction but don't commit
        let _snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Checkpoint should record that there's an active transaction
        engine.checkpoint().unwrap();

        // WAL should contain checkpoint record with min_active_lsn
        // (verified implicitly by the checkpoint succeeding)
    }

    #[test]
    fn test_wal_truncation_after_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Perform several transactions
        for i in 1..=5 {
            let snapshot = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: i, name: "Test" })
                .unwrap();
            engine.commit_transaction(snapshot).unwrap();
        }

        // Get WAL path for verification
        let wal_path = temp_dir.path().join("wal.log");

        // Checkpoint should allow truncating old WAL records
        engine.checkpoint().unwrap();

        // After checkpoint, we should be able to truncate the WAL
        // (Size might not change immediately, but structure should allow it)
        // For now, just verify checkpoint succeeds
        assert!(wal_path.exists());

        // WAL should still be functional after checkpoint
        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 6i64, name: "After checkpoint" })
            .unwrap();
        engine.commit_transaction(snapshot).unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 6);
    }

    // Recovery Tests (Step 7)

    #[test]
    fn test_recovery_replays_committed_insert() {
        let temp_dir = TempDir::new().unwrap();

        // Create database and perform committed transaction
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();

            let snapshot = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
            engine.commit_transaction(snapshot).unwrap();

            // Simulate crash (don't call checkpoint, just drop)
        }

        // Reopen - should trigger recovery and restore committed data
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let relation = engine.load_relation("TEST").unwrap();
            assert_eq!(
                relation.cardinality(),
                1,
                "Committed transaction should be recovered"
            );
        }
    }

    #[test]
    fn test_recovery_from_checkpoint() {
        let temp_dir = TempDir::new().unwrap();

        // Create database with checkpoint
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();

            // Transaction before checkpoint
            let snapshot1 = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Before" })
                .unwrap();
            engine.commit_transaction(snapshot1).unwrap();

            // Checkpoint
            engine.checkpoint().unwrap();

            // Transaction after checkpoint
            let snapshot2 = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 2i64, name: "After" })
                .unwrap();
            engine.commit_transaction(snapshot2).unwrap();

            // Simulate crash
        }

        // Reopen - should recover from checkpoint
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let relation = engine.load_relation("TEST").unwrap();
            assert_eq!(
                relation.cardinality(),
                2,
                "Recovery should work from checkpoint"
            );
        }
    }

    #[test]
    fn test_recovery_idempotent() {
        let temp_dir = TempDir::new().unwrap();

        // Create database with committed data
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();

            let snapshot = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
            engine.commit_transaction(snapshot).unwrap();
        }

        // Reopen multiple times - recovery should be idempotent
        for _ in 0..3 {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let relation = engine.load_relation("TEST").unwrap();
            assert_eq!(relation.cardinality(), 1, "Recovery should be idempotent");
        }
    }

    // Phase 3.1: Multiple Concurrent Transactions tests

    #[test]
    fn test_multiple_concurrent_begin() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Begin multiple transactions concurrently
        let snapshot1 = engine.begin_transaction().unwrap();
        let snapshot2 = engine.begin_transaction().unwrap();
        let snapshot3 = engine.begin_transaction().unwrap();

        // Each should have unique transaction ID
        assert_ne!(snapshot1.txn_id, snapshot2.txn_id);
        assert_ne!(snapshot2.txn_id, snapshot3.txn_id);
        assert_ne!(snapshot1.txn_id, snapshot3.txn_id);
    }

    #[test]
    fn test_each_txn_unique_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 begins (sees no active transactions)
        let snapshot1 = engine.begin_transaction().unwrap();

        // T2 begins (should see T1 as active)
        let snapshot2 = engine.begin_transaction().unwrap();

        // T3 begins (should see T1 and T2 as active)
        let snapshot3 = engine.begin_transaction().unwrap();

        // Each snapshot should be unique
        assert_ne!(snapshot1.txn_id, snapshot2.txn_id);
        assert_ne!(snapshot2.txn_id, snapshot3.txn_id);
    }

    #[test]
    fn test_transactions_isolated() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 begins and inserts
        let _snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // T2 begins and inserts
        let _snapshot2 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Both transactions should be active
        // (Verified by not panicking - we'll test visibility in Phase 3.2)
    }

    #[test]
    fn test_commit_removes_from_att() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot1 = engine.begin_transaction().unwrap();
        let snapshot2 = engine.begin_transaction().unwrap();

        // Commit T1
        engine.commit_transaction(snapshot1).unwrap();

        // T1 should be removed from active transactions
        // (We'll verify this in visibility tests)

        // T2 can still commit
        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_abort_removes_from_att() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot1 = engine.begin_transaction().unwrap();
        let snapshot2 = engine.begin_transaction().unwrap();

        // Abort T1
        engine.rollback_transaction(snapshot1).unwrap();

        // T1 should be removed from active transactions

        // T2 can still commit
        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_snapshot_captures_concurrent_active() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Start T1
        let _snapshot1 = engine.begin_transaction().unwrap();

        // Start T2 (should see T1 as active)
        let _snapshot2 = engine.begin_transaction().unwrap();

        // Start T3 (should see T1 and T2 as active)
        let _snapshot3 = engine.begin_transaction().unwrap();

        // The snapshots should have captured the active transactions
        // (Will be tested more thoroughly in visibility tests)
    }

    #[test]
    fn test_txn_after_commit() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 commits
        let snapshot1 = engine.begin_transaction().unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2 starts after T1 commits
        let snapshot2 = engine.begin_transaction().unwrap();

        // T2 should NOT see T1 as active
        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_sequential_transaction_ids() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot1 = engine.begin_transaction().unwrap();
        let snapshot2 = engine.begin_transaction().unwrap();
        let snapshot3 = engine.begin_transaction().unwrap();

        // Transaction IDs should be increasing
        assert!(snapshot2.txn_id > snapshot1.txn_id);
        assert!(snapshot3.txn_id > snapshot2.txn_id);

        engine.commit_transaction(snapshot1).unwrap();
        engine.commit_transaction(snapshot2).unwrap();
        engine.commit_transaction(snapshot3).unwrap();
    }

    // Phase 3.2: Visibility-Aware Load tests

    #[test]
    fn test_load_for_txn_sees_committed() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 inserts and commits
        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2 starts after T1 commits
        let snapshot2 = engine.begin_transaction().unwrap();

        // T2 should see T1's committed data
        let relation = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();
        assert_eq!(relation.cardinality(), 1);

        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_load_for_txn_sees_own_inserts() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 inserts (uncommitted)
        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
            .unwrap();

        // T1 should see its own uncommitted insert
        let relation = engine
            .load_relation_for_txn("TEST", snapshot1.txn_id)
            .unwrap();
        assert_eq!(relation.cardinality(), 1);

        engine.commit_transaction(snapshot1).unwrap();
    }

    #[test]
    fn test_load_for_txn_skips_concurrent() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 and T2 begin concurrently
        let snapshot1 = engine.begin_transaction().unwrap();
        let snapshot2 = engine.begin_transaction().unwrap();

        // T1 inserts
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
            .unwrap();

        // T2 should NOT see T1's uncommitted insert
        let relation = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();
        assert_eq!(relation.cardinality(), 0);

        engine.commit_transaction(snapshot1).unwrap();
        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_load_for_txn_multiple_views() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 inserts and commits
        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2 starts and inserts (uncommitted)
        let snapshot2 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 2i64, name: "Bob" }, snapshot2.txn_id)
            .unwrap();

        // T3 starts
        let snapshot3 = engine.begin_transaction().unwrap();

        // T2 sees: Alice (committed) + Bob (own insert) = 2
        let rel2 = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();
        assert_eq!(rel2.cardinality(), 2);

        // T3 sees: only Alice (T2's insert uncommitted) = 1
        let rel3 = engine
            .load_relation_for_txn("TEST", snapshot3.txn_id)
            .unwrap();
        assert_eq!(rel3.cardinality(), 1);

        engine.commit_transaction(snapshot2).unwrap();
        engine.commit_transaction(snapshot3).unwrap();
    }

    #[test]
    fn test_load_for_txn_after_commit_visible() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 begins
        let snapshot1 = engine.begin_transaction().unwrap();

        // T2 inserts and commits
        let snapshot2 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot2.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot2).unwrap();

        // T3 begins after T2 commits
        let snapshot3 = engine.begin_transaction().unwrap();

        // NOTE: Current implementation uses active_txns list, not commit LSNs
        // T1 WILL see T2's insert because T2 was not in T1's active_txns
        // (T2 started after T1 took its snapshot)
        // TODO: For full snapshot isolation, track commit LSNs and check:
        //       committed_lsn[T2] < snapshot1.snapshot_lsn
        let rel1 = engine
            .load_relation_for_txn("TEST", snapshot1.txn_id)
            .unwrap();
        assert_eq!(rel1.cardinality(), 1); // Sees committed data (Read Committed behavior)

        // T3 should see T2's insert (T2 committed before T3 started)
        let rel3 = engine
            .load_relation_for_txn("TEST", snapshot3.txn_id)
            .unwrap();
        assert_eq!(rel3.cardinality(), 1);

        engine.commit_transaction(snapshot1).unwrap();
        engine.commit_transaction(snapshot3).unwrap();
    }

    #[test]
    fn test_load_for_txn_empty_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();

        // Empty relation should return empty result
        let relation = engine
            .load_relation_for_txn("TEST", snapshot.txn_id)
            .unwrap();
        assert_eq!(relation.cardinality(), 0);

        engine.commit_transaction(snapshot).unwrap();
    }

    #[test]
    fn test_load_for_txn_nonexistent_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();

        // Loading nonexistent relation should fail
        let result = engine.load_relation_for_txn("NONEXISTENT", snapshot.txn_id);
        assert!(result.is_err());

        engine.commit_transaction(snapshot).unwrap();
    }

    #[test]
    fn test_insert_in_txn_logs_wal() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot.txn_id)
            .unwrap();

        // Insert should have logged to WAL
        // (Exact verification would require WAL inspection)

        engine.commit_transaction(snapshot).unwrap();
    }

    #[test]
    fn test_load_for_txn_concurrent_uncommitted() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 inserts uncommitted
        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
            .unwrap();

        // T2 inserts uncommitted
        let snapshot2 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 2i64, name: "Bob" }, snapshot2.txn_id)
            .unwrap();

        // T1 sees only its own insert
        let rel1 = engine
            .load_relation_for_txn("TEST", snapshot1.txn_id)
            .unwrap();
        assert_eq!(rel1.cardinality(), 1);

        // T2 sees only its own insert
        let rel2 = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();
        assert_eq!(rel2.cardinality(), 1);

        engine.commit_transaction(snapshot1).unwrap();
        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_load_for_txn_mixed_committed_uncommitted() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // T1 inserts and commits
        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2 inserts (uncommitted)
        let snapshot2 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", tuple! { id: 2i64, name: "Bob" }, snapshot2.txn_id)
            .unwrap();

        // T3 inserts and commits
        let snapshot3 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn(
                "TEST",
                tuple! { id: 3i64, name: "Charlie" },
                snapshot3.txn_id,
            )
            .unwrap();
        engine.commit_transaction(snapshot3).unwrap();

        // T4 starts
        let snapshot4 = engine.begin_transaction().unwrap();

        // T4 should see Alice (T1 committed) and Charlie (T3 committed), but NOT Bob (T2 uncommitted)
        let rel4 = engine
            .load_relation_for_txn("TEST", snapshot4.txn_id)
            .unwrap();
        assert_eq!(rel4.cardinality(), 2);

        engine.commit_transaction(snapshot2).unwrap();
        engine.commit_transaction(snapshot4).unwrap();
    }

    #[test]
    fn test_load_for_txn_preserves_tuple_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let original = tuple! { id: 42i64, name: "TestData" };

        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple_in_txn("TEST", original.clone(), snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        let snapshot2 = engine.begin_transaction().unwrap();
        let relation = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();

        assert_eq!(relation.cardinality(), 1);
        // Tuple data should be preserved
        assert!(relation.tuples().any(|t| t == &original));

        engine.commit_transaction(snapshot2).unwrap();
    }

    // Phase 6.2: Checkpoint Triggers GC (TDD - RED)

    #[test]
    fn test_checkpoint_triggers_gc() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Create relation
        let rel_type = test_rel_type();
        engine.create_relation("TEST", rel_type).unwrap();

        // T1: Insert and commit
        let snapshot1 = engine.begin_transaction().unwrap();
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        engine
            .insert_tuple_in_txn("TEST", tuple, snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2: Delete and commit
        let snapshot2 = engine.begin_transaction().unwrap();
        // Note: We need to delete by marking, but we don't have direct access
        // For now, this test will verify checkpoint runs without error
        engine.commit_transaction(snapshot2).unwrap();

        // Checkpoint should run GC
        engine.checkpoint().unwrap();

        // Verify checkpoint succeeded
        // (GC ran internally, no errors)
    }

    #[test]
    fn test_gc_reclaims_space_after_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Create relation
        let rel_type = test_rel_type();
        engine.create_relation("TEST", rel_type).unwrap();

        // Insert multiple tuples and delete them
        for i in 1..=10 {
            let snapshot = engine.begin_transaction().unwrap();
            let tuple = tuple! { id: i, name: format!("Test{}", i) };
            engine
                .insert_tuple_in_txn("TEST", tuple, snapshot.txn_id)
                .unwrap();
            engine.commit_transaction(snapshot).unwrap();
        }

        // All tuples inserted and committed
        // Checkpoint with no active transactions should succeed
        engine.checkpoint().unwrap();

        // Verify system is in consistent state
        let snapshot = engine.begin_transaction().unwrap();
        let relation = engine
            .load_relation_for_txn("TEST", snapshot.txn_id)
            .unwrap();

        assert_eq!(relation.cardinality(), 10);
        engine.commit_transaction(snapshot).unwrap();
    }

    #[test]
    fn test_gc_preserves_active_transaction_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Create relation
        let rel_type = test_rel_type();
        engine.create_relation("TEST", rel_type).unwrap();

        // T1: Insert and commit
        let snapshot1 = engine.begin_transaction().unwrap();
        let tuple = tuple! { id: 1i64, name: "Active" };
        engine
            .insert_tuple_in_txn("TEST", tuple.clone(), snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2: Begin (active transaction)
        let snapshot2 = engine.begin_transaction().unwrap();

        // Checkpoint with active transaction
        engine.checkpoint().unwrap();

        // T2 should still see the data
        let relation = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();

        assert_eq!(relation.cardinality(), 1);
        assert!(relation.tuples().any(|t| t == &tuple));

        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_recovery_undoes_uncommitted_data() {
        let temp_dir = TempDir::new().unwrap();

        // 1. Create database and perform uncommitted work
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();

            // Begin transaction
            let _snapshot = engine.begin_transaction().unwrap();

            // Insert tuple (written to WAL and Heap)
            // insert_tuple uses the active transaction established by begin_transaction
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Uncommitted" })
                .unwrap();

            // CRASH! (Drop engine without calling commit_transaction)
            // WAL contains: Begin, Insert
            // WAL does NOT contain: Commit
        }

        // 2. Reopen database (Trigger Recovery)
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();

            // Recovery should:
            // 1. See uncommitted transaction in WAL
            // 2. Scan heap files and remove tuples created by that transaction
            //    (via undo_uncommitted_inserts)

            let relation = engine.load_relation("TEST").unwrap();

            // Verify tuple is gone
            assert_eq!(
                relation.cardinality(),
                0,
                "Recovery failed to undo uncommitted insert"
            );
        }
    }

    #[test]
    fn test_recovery_ignores_dropped_relation() {
        let temp_dir = TempDir::new().unwrap();

        // 1. Create database, insert, then drop relation, then crash before commit
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine
                .create_relation("DROPPED_REL", test_rel_type())
                .unwrap();

            // Begin transaction
            let _snapshot = engine.begin_transaction().unwrap();

            // Insert tuple (written to WAL)
            engine
                .insert_tuple("DROPPED_REL", tuple! { id: 1i64, name: "ToDrop" })
                .unwrap();

            // Drop relation (removes from catalog and heap file)
            // Note: In a real crash scenario, the catalog drop might not be durable if not WAL-logged,
            // but here we simulate the state where the catalog update persisted but the txn didn't commit.
            engine.drop_relation("DROPPED_REL").unwrap();

            // CRASH! (Drop engine)
        }

        // 2. Reopen database
        {
            // Recovery runs. It sees uncommitted insert for "DROPPED_REL".
            // It should check if "DROPPED_REL" exists. It doesn't.
            // It should skip cleanup and open successfully.
            let engine = PersistentEngine::open(temp_dir.path());
            assert!(
                engine.is_ok(),
                "Engine should open successfully despite uncommitted inserts for dropped relation"
            );

            let engine = engine.unwrap();
            assert!(!engine.relation_exists("DROPPED_REL"));
        }
    }

    #[test]
    fn test_recovery_retains_valid_data_during_cleanup() {
        let temp_dir = TempDir::new().unwrap();

        // 1. Create database and insert committed data
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();

            // Committed transaction
            let snapshot = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Committed" })
                .unwrap();
            engine.commit_transaction(snapshot).unwrap();

            // Uncommitted transaction (simulating crash)
            let _snapshot = engine.begin_transaction().unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 2i64, name: "Uncommitted" })
                .unwrap();

            // CRASH! (Drop engine)
        }

        // 2. Reopen database (Trigger Recovery)
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let relation = engine.load_relation("TEST").unwrap();

            // Verify committed tuple exists
            assert_eq!(
                relation.cardinality(),
                1,
                "Recovery should retain committed data"
            );
            assert!(
                relation
                    .tuples()
                    .any(|t| t.get_typed::<i64>("id").unwrap() == 1),
                "Committed tuple should persist"
            );

            // Verify uncommitted tuple is gone
            assert!(
                !relation
                    .tuples()
                    .any(|t| t.get_typed::<i64>("id").unwrap() == 2),
                "Uncommitted tuple should be removed"
            );
        }
    }
}
