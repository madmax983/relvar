//! Persistent storage engine implementation using heap files and catalog.

use crate::mvcc::{
    DEFAULT_TXN_POOL_CAPACITY, DEFAULT_VERSION_BUFFER_CAPACITY, DEFAULT_VERSION_POOL_CAPACITY,
    TxnPool, VersionPool,
};
use crate::storage::StorageManager;
use crate::sync::RwLock;
use crate::wal::{TransactionId, TransactionIdGenerator, WalManager, WalRecord, recover};
use relvar_core::storage_engine::{IsolationLevel, RelationMetadata, StorageEngine, StorageError};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Per-transaction isolation bookkeeping.
///
/// Records what a live transaction has observed and changed so that
/// [`IsolationLevel::Serializable`] commit validation (first-committer-wins)
/// can detect overlapping concurrent transactions.
#[derive(Debug)]
struct TxnState {
    /// The isolation level the transaction began with.
    level: IsolationLevel,
    /// Length of the engine's commit log when this transaction began.
    /// Transactions committed at or after this index are concurrent with it.
    begin_commit_index: usize,
    /// Relations this transaction has read.
    read_relations: HashSet<String>,
    /// Relations this transaction has written (DML).
    written_relations: HashSet<String>,
}

/// Mutable isolation bookkeeping shared across the engine's read and write paths.
///
/// Kept behind a lock because the read path (`load_relation`) only has `&self`.
#[derive(Debug, Default)]
struct TxnTracker {
    /// Live-transaction bookkeeping, keyed by transaction id.
    states: HashMap<TransactionId, TxnState>,
    /// Committed writers per relation, in commit order. Only transactions
    /// that committed appear here; rolled-back writers never do.
    relation_writers: HashMap<String, Vec<TransactionId>>,
    /// Transaction ids in commit order.
    commit_order: Vec<TransactionId>,
}

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
///   transactions to read and write simultaneously without locking. What each
///   transaction observes is governed by its [`IsolationLevel`]:
///   [`IsolationLevel::ReadCommitted`] re-reads the latest committed state on
///   every read, while [`IsolationLevel::RepeatableRead`] and
///   [`IsolationLevel::Serializable`] observe a fixed snapshot taken when the
///   transaction begins. Serializable additionally validates at commit time
///   that no concurrent transaction committed overlapping changes
///   (first-committer-wins); the loser is aborted with
///   [`StorageError::SerializationFailure`]. Writes create new versions of
///   tuples rather than overwriting them in place.
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
    txn_pool: TxnPool,
    /// Set of committed transaction IDs for visibility checks.
    committed_txns: HashSet<TransactionId>,
    /// Current transaction context for operations.
    current_txn: Option<TransactionId>,
    /// Isolation bookkeeping for live transactions (read/write sets,
    /// commit order). Behind a lock because reads only have `&self`.
    txn_tracker: RwLock<TxnTracker>,
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
        Self::open_with_capacity(path, DEFAULT_TXN_POOL_CAPACITY)
    }

    /// Opens a database with a bounded transaction pool of `txn_pool_capacity`
    /// concurrently live transactions.
    ///
    /// [`PersistentEngine::open`] uses [`DEFAULT_TXN_POOL_CAPACITY`]. Pass a
    /// smaller bound on memory-constrained targets; beginning past it fails
    /// with a typed
    /// [`StorageError::TransactionPoolExhausted`](relvar_core::storage_engine::StorageError::TransactionPoolExhausted)
    /// instead of growing past it.
    ///
    /// The version pool uses [`DEFAULT_VERSION_POOL_CAPACITY`] records and
    /// [`DEFAULT_VERSION_BUFFER_CAPACITY`] page working buffers; see
    /// [`open_with_pool_capacities`](Self::open_with_pool_capacities) to tune
    /// it.
    ///
    /// # Errors
    ///
    /// Yields an error if I/O operations fail.
    pub fn open_with_capacity<P: AsRef<Path>>(
        path: P,
        txn_pool_capacity: usize,
    ) -> Result<Self, StorageError> {
        Self::open_with_pool_capacities(
            path,
            txn_pool_capacity,
            DEFAULT_VERSION_POOL_CAPACITY,
            DEFAULT_VERSION_BUFFER_CAPACITY,
        )
    }

    /// Opens a database with bounded transaction and version pools.
    ///
    /// `version_pool_capacity` bounds the number of concurrently uncommitted
    /// tuple versions; `version_buffer_capacity` bounds the checked-out page
    /// working buffers. Exhausting either fails the write with a typed
    /// [`StorageError::VersionPoolExhausted`](relvar_core::storage_engine::StorageError::VersionPoolExhausted)
    /// or
    /// [`StorageError::VersionBufferExhausted`](relvar_core::storage_engine::StorageError::VersionBufferExhausted)
    /// instead of allocating past the bound or panicking.
    ///
    /// The version pool's claimant table is sized from `txn_pool_capacity`:
    /// outstanding claims always belong to live transactions, so the bound
    /// can never trigger spuriously.
    ///
    /// # Errors
    ///
    /// Yields an error if I/O operations fail.
    pub fn open_with_pool_capacities<P: AsRef<Path>>(
        path: P,
        txn_pool_capacity: usize,
        version_pool_capacity: usize,
        version_buffer_capacity: usize,
    ) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();
        let wal_path = db_path.join("wal.log");

        // Initialize StorageManager (handles DB dir and catalog) with a
        // caller-owned, fixed-capacity version pool.
        let version_pool = VersionPool::with_capacities(
            version_pool_capacity,
            version_buffer_capacity,
            txn_pool_capacity,
        );
        let storage_manager = StorageManager::with_version_pool(&db_path, version_pool)?;

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
            txn_pool: TxnPool::new(txn_pool_capacity),
            committed_txns: recovery_result.committed_txns, // Populate from recovery
            current_txn: None,
            txn_tracker: RwLock::new(TxnTracker::default()),
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
                .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .scan_relation(
                relation_name,
                &snapshot,
                &self.txn_pool,
                &self.committed_txns,
            )?;

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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .flush_heap_files()?;

        // Determine minimum active LSN
        let min_active_lsn = self.get_checkpoint_lsn();

        // Log checkpoint record
        self.log_checkpoint_record(min_active_lsn)?;

        // Flush WAL to ensure checkpoint is durable
        self.flush_wal()?;

        // Garbage collect old versions. The pool resolves each deleter's
        // commit LSN, so reclamation stays constrained by live snapshots.
        let gc_lsn = self.get_checkpoint_lsn();
        let txn_pool = &self.txn_pool;
        let committed_txns = &self.committed_txns;
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .garbage_collect_versions(gc_lsn, committed_txns, txn_pool)?;

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
        self.txn_pool
            .oldest_active_lsn()
            .unwrap_or_else(|| self.wal.current_lsn())
    }

    /// Log checkpoint record to WAL.
    fn log_checkpoint_record(
        &mut self,
        min_active_lsn: crate::wal::Lsn,
    ) -> Result<(), StorageError> {
        // Collect dirty pages logic is simplified - assume all managed files are potentially dirty
        // A real implementation would track dirty pages in StorageManager.
        // (`WalRecord::Checkpoint` holds a no_std hashbrown map, so the empty
        // map is inferred from the expected type.)
        self.wal
            .log(WalRecord::Checkpoint {
                min_active_lsn,
                dirty_pages: Default::default(),
            })
            .map_err(|e| StorageError::Other(format!("WAL checkpoint error: {}", e)))?;
        Ok(())
    }

    /// Get snapshot for the current transaction context.
    ///
    /// The snapshot honors the ambient transaction's isolation level:
    /// [`IsolationLevel::ReadCommitted`] builds a fresh snapshot on every
    /// call (latest committed state), while the stronger levels reuse the
    /// snapshot captured when the transaction began.
    fn get_snapshot_for_current_context(
        &self,
    ) -> Result<crate::mvcc::TransactionSnapshot, StorageError> {
        if let Some(txn_id) = self.current_txn {
            self.snapshot_for_txn(txn_id)
        } else {
            // Outside transaction - see all committed data
            Ok(
                crate::mvcc::TransactionSnapshot::new(
                    TransactionId::new(0),
                    self.wal.current_lsn(),
                )
                .with_horizon(self.txn_id_gen.peek_next()),
            )
        }
    }

    /// Get the read snapshot for an explicit transaction id.
    ///
    /// This is the level-aware core shared by the ambient read path and
    /// [`load_relation_for_txn`](Self::load_relation_for_txn).
    fn snapshot_for_txn(
        &self,
        txn_id: TransactionId,
    ) -> Result<crate::mvcc::TransactionSnapshot, StorageError> {
        let level = self
            .txn_tracker
            .read()
            .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?
            .states
            .get(&txn_id)
            .map(|state| state.level)
            .unwrap_or(IsolationLevel::RepeatableRead);

        match level {
            IsolationLevel::ReadCommitted => {
                // Fresh snapshot on every read: observe the latest committed
                // state. The snapshot is a Copy value — no active-set copy.
                // Liveness is resolved against the pool at visibility time:
                // other live transactions' uncommitted changes stay invisible
                // (the transaction itself is excluded so its own writes stay
                // visible). The horizon is the current generator value:
                // transactions that begin after this read are invisible to it.
                Ok(
                    crate::mvcc::TransactionSnapshot::new(txn_id, self.wal.current_lsn())
                        .with_horizon(self.txn_id_gen.peek_next()),
                )
            }
            IsolationLevel::RepeatableRead | IsolationLevel::Serializable => {
                // Fixed begin snapshot: repeatable reads for the whole transaction.
                self.txn_pool.get_snapshot(txn_id).ok_or_else(|| {
                    StorageError::Other(format!("Transaction {} not found", txn_id.value()))
                })
            }
        }
    }

    /// Records that `txn_id` read `relation_name` (serializable read set).
    fn note_read(&self, txn_id: TransactionId, relation_name: &str) -> Result<(), StorageError> {
        let mut tracker = self
            .txn_tracker
            .write()
            .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?;
        if let Some(state) = tracker.states.get_mut(&txn_id) {
            state.read_relations.insert(relation_name.to_string());
        }
        Ok(())
    }

    /// Records that `txn_id` wrote `relation_name` (serializable write set).
    fn note_write(&self, txn_id: TransactionId, relation_name: &str) -> Result<(), StorageError> {
        let mut tracker = self
            .txn_tracker
            .write()
            .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?;
        if let Some(state) = tracker.states.get_mut(&txn_id) {
            state.written_relations.insert(relation_name.to_string());
        }
        Ok(())
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .create_relation(name, relation_type)
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .get_relation_metadata(name)
    }

    fn list_relations(&self) -> Vec<String> {
        self.storage_manager
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .list_relations()
    }

    fn load_relation(&self, name: &str) -> Result<Relation, StorageError> {
        // Use MVCC visibility if in a transaction, otherwise see all committed data.
        // The explicit-txn path applies the transaction's isolation level and
        // records the read for serializable validation.
        if let Some(txn_id) = self.current_txn {
            self.load_relation_for_txn(name, txn_id)
        } else {
            let snapshot = self.get_snapshot_for_current_context()?;
            self.storage_manager
                .write()
                .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
                .scan_relation(name, &snapshot, &self.txn_pool, &self.committed_txns)
        }
    }

    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        let (txn_id, auto_commit) = self.ensure_transaction()?;

        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .store_relation(name, relation, txn_id)?;

        self.note_write(txn_id, name)?;

        if auto_commit {
            let snapshot = PersistentSnapshot { txn_id };
            self.commit_transaction(snapshot)?;
        }

        Ok(())
    }

    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        let (txn_id, auto_commit) = self.ensure_transaction()?;

        self.insert_tuple_in_txn(name, tuple, txn_id)?;

        if auto_commit {
            let snapshot = PersistentSnapshot { txn_id };
            self.commit_transaction(snapshot)?;
        }

        Ok(())
    }

    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError> {
        self.begin_transaction_with_isolation(IsolationLevel::default())
    }

    fn begin_transaction_with_isolation(
        &mut self,
        level: IsolationLevel,
    ) -> Result<Self::Snapshot, StorageError> {
        // Beginning a transaction makes it the ambient context. A previously
        // ambient transaction stays live — it is only displaced, not ended —
        // and remains committable through its own snapshot handle. Use
        // `suspend_transaction` when the displacement must be explicit.
        let txn_id = self.txn_id_gen.generate();
        let current_lsn = self.wal.current_lsn();
        // Visibility horizon: every transaction that has begun so far holds
        // an ID below this value, so versions created by transactions that
        // begin after this snapshot are invisible to it.
        let horizon = self.txn_id_gen.peek_next();

        // Claim the pool slot BEFORE writing the WAL record. If the pool is
        // exhausted we fail typed without leaving a logged begin behind; if
        // the WAL write then fails we release the slot again so no live
        // transaction is stranded.
        self.txn_pool
            .begin(txn_id, current_lsn, horizon)
            .map_err(StorageError::from)?;
        if let Err(e) = self.wal.log(WalRecord::Begin { txn_id }) {
            let _ = self.txn_pool.abort(txn_id);
            return Err(StorageError::Other(format!("WAL error: {e}")));
        }
        // The Begin record must be durable before the transaction writes
        // anything: recovery seeds the transaction-ID generator from the
        // WAL, and a Begin lost to the log buffer would let a later engine
        // reopen reuse this ID — resurrecting this transaction's
        // uncommitted versions as the new transaction's own writes. A
        // flush failure releases the pool slot like a log failure does, so
        // no live transaction is stranded.
        if let Err(e) = self.wal.flush() {
            let _ = self.txn_pool.abort(txn_id);
            return Err(StorageError::Other(format!("WAL flush error: {e}")));
        }

        let mut tracker = self
            .txn_tracker
            .write()
            .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?;
        let begin_commit_index = tracker.commit_order.len();
        tracker.states.insert(
            txn_id,
            TxnState {
                level,
                begin_commit_index,
                read_relations: HashSet::new(),
                written_relations: HashSet::new(),
            },
        );

        self.current_txn = Some(txn_id);

        Ok(PersistentSnapshot { txn_id })
    }

    fn commit_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        let txn_id = snapshot.txn_id;

        // Serializable validation runs before anything is made durable: a
        // conflict aborts the transaction instead of committing it.
        match self.serializable_conflict(txn_id) {
            Ok(Some(reason)) => {
                self.abort_txn(txn_id)?;
                return Err(StorageError::SerializationFailure(reason));
            }
            Ok(None) => {}
            Err(error) => {
                // Validation itself failed (e.g. a poisoned lock): end the
                // transaction best-effort rather than strand it, then report
                // the original failure.
                let _ = self.abort_txn(txn_id);
                return Err(error);
            }
        }

        // `commit_txn_durable` ends the transaction itself on every failure
        // path — aborted before the commit point, or committed when the
        // commit-point flush left the outcome uncertain — so the trait
        // contract ("on Err the transaction is no longer active") holds
        // without a second abort here (which would only append a duplicate
        // Abort record).
        self.commit_txn_durable(snapshot)?;
        Ok(())
    }

    fn rollback_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        self.abort_txn(snapshot.txn_id)
    }
}

/// Serializable (first-committer-wins) validation and abort plumbing.
impl PersistentEngine {
    /// Checks whether committing `txn_id` would violate serializability.
    ///
    /// Returns `Some(reason)` when a transaction that committed after this
    /// one began wrote to a relation this transaction read or wrote.
    /// Conflict detection is at relation granularity: any overlap aborts,
    /// which is conservative (it may abort transactions that would not
    /// actually conflict at tuple granularity) but always safe.
    ///
    /// Schema (DDL) operations are intentionally outside this validation:
    /// the engine versions tuples, not the catalog, so creating or dropping
    /// a relation does not join any transaction's read/write set.
    ///
    /// Returns `Ok(None)` for non-serializable levels, for unknown
    /// transactions, and when no concurrent committed writer overlaps.
    fn serializable_conflict(&self, txn_id: TransactionId) -> Result<Option<String>, StorageError> {
        let tracker = self
            .txn_tracker
            .read()
            .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?;
        let Some(state) = tracker.states.get(&txn_id) else {
            return Ok(None);
        };
        if state.level != IsolationLevel::Serializable {
            return Ok(None);
        }
        // Transactions that committed while this one was running.
        let concurrent: HashSet<TransactionId> = tracker.commit_order[state.begin_commit_index..]
            .iter()
            .copied()
            .collect();
        if concurrent.is_empty() {
            return Ok(None);
        }
        for relation in state
            .read_relations
            .iter()
            .chain(state.written_relations.iter())
        {
            if let Some(writers) = tracker.relation_writers.get(relation) {
                for writer in writers {
                    if *writer != txn_id && concurrent.contains(writer) {
                        return Ok(Some(format!(
                            "concurrent transaction {} committed changes to relation {relation} \
                             after this transaction began",
                            writer.value(),
                        )));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Aborts `txn_id`: logs the abort, drops its MVCC snapshot and
    /// isolation bookkeeping, and clears the ambient context when it was
    /// this transaction.
    ///
    /// The Abort record is logged BEFORE any in-memory state mutates: a
    /// WAL failure then leaves the transaction live and retryable instead
    /// of half-aborted. (If the Abort record itself never reaches the
    /// WAL, recovery still treats the transaction as aborted — a Begin
    /// with no Commit replays as aborted — so the lost record resolves in
    /// the safe direction.)
    ///
    /// Aborted tuples stay in the heap but are invisible: their creating
    /// transaction never joins `committed_txns`, so MVCC visibility rules
    /// hide them from every other transaction (they are reclaimed by GC).
    fn abort_txn(&mut self, txn_id: TransactionId) -> Result<(), StorageError> {
        self.wal
            .log(WalRecord::Abort { txn_id })
            .map_err(|e| StorageError::Other(format!("WAL error: {e}")))?;

        // Recycle the transaction's pool slot. A history-ring exhaustion
        // fails typed while the transaction is still live and retryable.
        self.txn_pool.abort(txn_id).map_err(StorageError::from)?;

        // Recycle the transaction's version records (infallible): its
        // versions stay on disk but invisible, for GC to reclaim.
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .release_versions_for_txn(txn_id);

        self.txn_tracker
            .write()
            .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?
            .states
            .remove(&txn_id);

        if self.current_txn == Some(txn_id) {
            self.current_txn = None;
        }

        Ok(())
    }

    /// Makes `snapshot`'s transaction durable: WAL commit record, heap flush,
    /// and MVCC bookkeeping.
    ///
    /// Ordering contract: the WAL commit record is the single commit
    /// point. No in-memory state claims the transaction is committed
    /// before that record is durable, so a failed commit can never leave
    /// the pool disagreeing with the WAL about whether the transaction
    /// committed (the old order mutated the pool first, and a WAL failure
    /// then stranded a "committed" history record behind a fallback abort
    /// that could not retract it).
    ///
    /// On every failure path the transaction is already ended when this
    /// returns `Err` — aborted before the commit point, or ended as
    /// committed when the commit-point flush left the outcome uncertain —
    /// satisfying the [`StorageEngine::commit_transaction`] contract
    /// without the caller needing a second abort.
    fn commit_txn_durable(&mut self, snapshot: PersistentSnapshot) -> Result<(), StorageError> {
        // 1. Pre-flight the history ring: `can_finish` dry-runs the
        //    post-commit pool finish. If the ring is exhausted the commit
        //    fails here — typed, transaction still live, nothing durable —
        //    instead of stranding a durable commit the pool cannot record.
        //    (Sound: the engine drives the pool single-threaded, so no
        //    finish can intervene before the real one in `complete_commit`.)
        if let Err(error) = self.txn_pool.can_finish(snapshot.txn_id) {
            let _ = self.abort_txn(snapshot.txn_id);
            return Err(StorageError::from(error));
        }

        // 2. Make all write records durable BEFORE heap pages go out
        //    (WAL-before-data): a crash must never leave heap pages whose
        //    log records are still in the buffer.
        if let Err(error) = self.wal.flush() {
            let _ = self.abort_txn(snapshot.txn_id);
            return Err(StorageError::Other(format!("WAL flush error: {error}")));
        }

        // 3. Flush heap pages while the commit is still undecided: a
        //    failure here aborts the transaction with nothing durable.
        //    (The lock guard drops at the end of this statement so the
        //    abort below can borrow `&mut self`.)
        let heap_flush = self
            .storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .flush_heap_files();
        if let Err(error) = heap_flush {
            let _ = self.abort_txn(snapshot.txn_id);
            return Err(error);
        }

        // The commit LSN is the pre-append WAL position — no snapshot can
        // be taken between here and the WAL append (`&mut self`), so every
        // later snapshot still orders after the commit.
        let commit_lsn = self.wal.current_lsn();

        // 4. Log the commit record. A failure here aborts with no commit
        //    record in the WAL.
        if let Err(error) = self.wal.log(WalRecord::Commit {
            txn_id: snapshot.txn_id,
        }) {
            let _ = self.abort_txn(snapshot.txn_id);
            return Err(StorageError::Other(format!("WAL error: {error}")));
        }

        // 5. THE COMMIT POINT: the commit record becomes durable. Past this
        //    line the transaction is committed — there is no path back to
        //    "live" or "aborted".
        if let Err(error) = self.wal.flush() {
            // The record may or may not have reached stable storage: the
            // outcome is unknowable, and the WAL is the source of truth.
            // The engine ends the transaction as COMMITTED — ending it as
            // aborted here could resurrect it as committed on recovery
            // (a Commit record replays as committed) while memory says
            // aborted. The error names the uncertainty: the caller must
            // reconcile, not blindly retry.
            self.complete_commit(snapshot, commit_lsn)?;
            return Err(StorageError::Other(format!(
                "WAL commit flush failed ({error}); commit outcome uncertain — \
                 transaction ended as committed, do not blindly retry"
            )));
        }

        // 6. Post-commit bookkeeping. Infallible by construction: the
        //    history ring was pre-flighted, the version release cannot
        //    fail, and the rest is in-memory (lock poisoning is a
        //    bug-state, reported as `Other` like everywhere else).
        self.complete_commit(snapshot, commit_lsn)
    }

    /// Ends `snapshot`'s transaction as committed in memory: pool history,
    /// committed set, isolation bookkeeping, ambient context, and version
    /// claim recycling.
    ///
    /// May only be called at or after the WAL commit point — the commit
    /// record is durable, or its durability is uncertain and the engine
    /// has chosen presumed-commit. There is no abort path back from here.
    /// The history-ring push cannot fail: the caller pre-flighted it with
    /// [`TxnPool::can_finish`].
    fn complete_commit(
        &mut self,
        snapshot: PersistentSnapshot,
        commit_lsn: crate::wal::Lsn,
    ) -> Result<(), StorageError> {
        self.txn_pool
            .commit(snapshot.txn_id, commit_lsn)
            .map_err(|e| {
                StorageError::Other(format!(
                    "internal error: pre-flighted transaction-pool commit failed: {e}"
                ))
            })?;

        self.committed_txns.insert(snapshot.txn_id);

        // Bookkeeping: the committed transaction becomes a committed writer
        // of everything it wrote, and joins the commit order used to detect
        // transactions concurrent with still-running ones.
        {
            let mut tracker = self
                .txn_tracker
                .write()
                .map_err(|_| StorageError::Other("txn tracker lock poisoned".to_string()))?;
            if let Some(state) = tracker.states.remove(&snapshot.txn_id) {
                for relation in state.written_relations {
                    tracker
                        .relation_writers
                        .entry(relation)
                        .or_default()
                        .push(snapshot.txn_id);
                }
            }
            tracker.commit_order.push(snapshot.txn_id);
        }

        if self.current_txn == Some(snapshot.txn_id) {
            self.current_txn = None;
        }

        // The transaction's versions are now committed: recycle its version
        // records. Infallible, so it cannot fail a durable commit.
        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .release_versions_for_txn(snapshot.txn_id);

        Ok(())
    }

    /// Resumes a previously suspended transaction as the ambient context.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Other`] when another transaction is already
    /// ambient, or when the suspended transaction is no longer live (it
    /// committed or aborted while suspended).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use relvar_storage::PersistentEngine;
    /// use relvar_core::storage_engine::{IsolationLevel, StorageEngine};
    /// use tempfile::tempdir;
    ///
    /// let dir = tempdir().unwrap();
    /// let mut engine = PersistentEngine::open(dir.path()).unwrap();
    /// let t1 = engine
    ///     .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
    ///     .unwrap();
    /// let suspended = engine.suspend_transaction().unwrap();
    /// // ... run another transaction here ...
    /// engine.resume_transaction(&suspended).unwrap();
    /// engine.commit_transaction(t1).unwrap();
    /// ```
    pub fn resume_transaction(
        &mut self,
        snapshot: &PersistentSnapshot,
    ) -> Result<(), StorageError> {
        if self.current_txn.is_some() {
            return Err(StorageError::Other(
                "a transaction is already active in this context".to_string(),
            ));
        }
        if self.txn_pool.get_snapshot(snapshot.txn_id).is_none() {
            return Err(StorageError::Other(format!(
                "transaction {} is no longer active",
                snapshot.txn_id.value()
            )));
        }
        self.current_txn = Some(snapshot.txn_id);
        Ok(())
    }

    /// Suspends the ambient transaction without committing or aborting it.
    ///
    /// The transaction stays live — its snapshot, isolation level, and
    /// read/write bookkeeping are kept — but it stops being the ambient
    /// context, so a different transaction can be begun in the meantime.
    /// Returns the suspended transaction's snapshot, or `None` when no
    /// transaction is ambient.
    ///
    /// This is how logically concurrent transactions interleave on this
    /// single-threaded engine: suspend T1, begin and commit T2, then
    /// [`resume_transaction`](Self::resume_transaction) T1.
    pub fn suspend_transaction(&mut self) -> Option<PersistentSnapshot> {
        self.current_txn
            .take()
            .map(|txn_id| PersistentSnapshot { txn_id })
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
        &self,
        name: &str,
        txn_id: TransactionId,
    ) -> Result<Relation, StorageError> {
        let snapshot = self.snapshot_for_txn(txn_id)?;
        self.note_read(txn_id, name)?;

        self.storage_manager
            .write()
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .scan_relation(name, &snapshot, &self.txn_pool, &self.committed_txns)
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .insert_tuple(name, tuple, txn_id)?;

        self.note_write(txn_id, name)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
