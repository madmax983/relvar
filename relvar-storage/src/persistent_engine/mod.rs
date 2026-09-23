//! Persistent storage engine implementation using heap files and catalog.

use crate::mvcc::ActiveTransactionTable;
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
    active_txns: ActiveTransactionTable,
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
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
            Ok(crate::mvcc::TransactionSnapshot::new(
                TransactionId::new(0),
                self.wal.current_lsn(),
                vec![],
            )
            .with_horizon(self.txn_id_gen.peek_next()))
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
                // state. Other active transactions stay in the active list so
                // their uncommitted changes remain invisible; the transaction
                // itself is excluded so its own writes stay visible. The
                // horizon is the current generator value: transactions that
                // begin after this read are invisible to it.
                let active: Vec<TransactionId> = self
                    .active_txns
                    .active_ids()
                    .into_iter()
                    .filter(|id| *id != txn_id)
                    .collect();
                Ok(
                    crate::mvcc::TransactionSnapshot::new(txn_id, self.wal.current_lsn(), active)
                        .with_horizon(self.txn_id_gen.peek_next()),
                )
            }
            IsolationLevel::RepeatableRead | IsolationLevel::Serializable => {
                // Fixed begin snapshot: repeatable reads for the whole transaction.
                self.active_txns
                    .get_snapshot(txn_id)
                    .cloned()
                    .ok_or_else(|| {
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
            .unwrap()
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
            .unwrap()
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
            self.storage_manager.write().unwrap().scan_relation(
                name,
                &snapshot,
                &self.committed_txns,
            )
        }
    }

    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        let (txn_id, auto_commit) = self.ensure_transaction()?;

        self.storage_manager
            .write()
            .unwrap()
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

        self.wal
            .log(WalRecord::Begin { txn_id })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        let _snapshot = self.active_txns.begin(txn_id, current_lsn, horizon);

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

        if let Err(error) = self.commit_txn_durable(snapshot) {
            // Contract (`StorageEngine::commit_transaction`): a failed commit
            // never strands a transaction. Abort best-effort so the engine is
            // left without an active transaction; if the abort itself fails
            // there is nothing further that can be done.
            let _ = self.abort_txn(txn_id);
            return Err(error);
        }
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
    /// Aborted tuples stay in the heap but are invisible: their creating
    /// transaction never joins `committed_txns`, so MVCC visibility rules
    /// hide them from every other transaction (they are reclaimed by GC).
    fn abort_txn(&mut self, txn_id: TransactionId) -> Result<(), StorageError> {
        self.wal
            .log(WalRecord::Abort { txn_id })
            .map_err(|e| StorageError::Other(format!("WAL error: {}", e)))?;

        self.active_txns.abort(txn_id);

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
    /// On error the caller is responsible for ending the transaction (see
    /// `commit_transaction`).
    fn commit_txn_durable(&mut self, snapshot: PersistentSnapshot) -> Result<(), StorageError> {
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
            .map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?
            .flush_heap_files()?;

        self.committed_txns.insert(snapshot.txn_id);
        self.active_txns.commit(snapshot.txn_id);

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
        if self.active_txns.get_snapshot(snapshot.txn_id).is_none() {
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
            .insert_tuple(name, tuple, txn_id)?;

        self.note_write(txn_id, name)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
