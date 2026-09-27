//! Storage Manager for physical data management.
//!
//! This module handles the physical storage layer operations, including:
//! - Catalog management (metadata)
//! - Heap file management (tuple storage)
//! - Garbage collection
//!
//! It abstracts the physical storage details from the transaction management layer.

use crate::FileBlockDevice;
use crate::mvcc::{DEFAULT_VERSION_POOL_CAPACITY, TransactionSnapshot, TxnPool, VersionPool};
use crate::storage::{Catalog, CatalogError, HeapError, HeapFile};
use crate::wal::TransactionId;
use relvar_core::storage_engine::{RelationMetadata, StorageError};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Storage Manager handling physical storage operations.
pub struct StorageManager {
    /// Base directory for database files.
    db_path: PathBuf,
    /// Path to the catalog file.
    catalog_path: PathBuf,
    /// System catalog containing relation metadata.
    catalog: Catalog,
    /// Open heap files, keyed by relation name.
    heap_files: HashMap<String, HeapFile<FileBlockDevice>>,
    /// Caller-owned, fixed-capacity MVCC version pool.
    ///
    /// All versioned heap operations (`insert`/`scan`/`gc`) claim version
    /// records and check out page working buffers from this pool instead of
    /// allocating per version. Exhaustion fails with a typed
    /// [`StorageError`](relvar_core::storage_engine::StorageError) and never
    /// panics.
    version_pool: VersionPool,
}

impl StorageManager {
    /// Create a new StorageManager instance.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        Self::with_version_pool(path, VersionPool::new(DEFAULT_VERSION_POOL_CAPACITY))
    }

    /// Create a new StorageManager with an explicit version pool.
    ///
    /// The pool is caller-owned and fixed-capacity: pass a smaller pool to
    /// bound memory on constrained targets. Exhaustion of the pool during
    /// versioned operations fails with a typed error, never a panic.
    pub fn with_version_pool<P: AsRef<Path>>(
        path: P,
        version_pool: VersionPool,
    ) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();

        if !db_path.exists() {
            std::fs::create_dir_all(&db_path)
                .map_err(|e| StorageError::Other(format!("Failed to create directory: {}", e)))?;
        }

        let catalog_path = db_path.join("catalog.json");

        let catalog = if catalog_path.exists() {
            Catalog::load(&catalog_path).map_err(Self::convert_catalog_error)?
        } else {
            Catalog::new()
        };

        Ok(Self {
            db_path,
            catalog_path,
            catalog,
            heap_files: HashMap::new(),
            version_pool,
        })
    }

    /// Save the catalog to disk.
    fn save_catalog(&self) -> Result<(), StorageError> {
        self.catalog
            .save(&self.catalog_path)
            .map_err(Self::convert_catalog_error)
    }

    /// Convert CatalogError to StorageError.
    fn convert_catalog_error(e: CatalogError) -> StorageError {
        match e {
            CatalogError::Io(io_err) => {
                StorageError::Other(format!("Catalog I/O error: {}", io_err))
            }
            CatalogError::Serialization(s) => {
                StorageError::Other(format!("Catalog serialization error: {}", s))
            }
            CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
        }
    }

    /// Convert HeapError to StorageError.
    ///
    /// MVCC pool exhaustion keeps its typed variant (never downgraded to
    /// `Other`): callers can match on it and retry after commit/abort.
    fn convert_heap_error(e: HeapError) -> StorageError {
        match e {
            HeapError::Mvcc(mvcc_err) => StorageError::from(mvcc_err),
            _ => StorageError::Other(format!("Heap error: {}", e)),
        }
    }

    /// Validate that a relvar name is safe for use in file paths.
    fn validate_relvar_name(name: &str) -> Result<(), StorageError> {
        if name.is_empty() {
            return Err(StorageError::Other(
                "Relvar name cannot be empty".to_string(),
            ));
        }

        if name.contains('/') || name.contains('\\') {
            return Err(StorageError::Other(format!(
                "Relvar name '{}' cannot contain path separators",
                name
            )));
        }

        if name == "." || name == ".." || name.contains("..") {
            return Err(StorageError::Other(format!(
                "Relvar name '{}' cannot contain parent directory references",
                name
            )));
        }

        Ok(())
    }

    /// Create a new relation.
    pub fn create_relation(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), StorageError> {
        Self::validate_relvar_name(name)?;

        if self.catalog.relation_exists(name) {
            return Err(StorageError::RelationAlreadyExists(name.to_string()));
        }

        // Create heap file path
        let heap_file_path = self.db_path.join(format!("{}.heap", name));

        // Create the heap file
        let heap_file = HeapFile::create(&heap_file_path, relation_type.clone())
            .map_err(Self::convert_heap_error)?;

        // Add to catalog
        self.catalog
            .create_relation(name.to_string(), relation_type, heap_file_path.clone())
            .map_err(Self::convert_catalog_error)?;

        // Save catalog
        self.save_catalog()?;

        // Cache the heap file
        self.heap_files.insert(name.to_string(), heap_file);

        Ok(())
    }

    /// Drop a relation.
    pub fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        // Remove from catalog
        let metadata = self
            .catalog
            .drop_relation(name)
            .map_err(Self::convert_catalog_error)?;

        // Save catalog
        self.save_catalog()?;

        // Remove heap file from cache
        self.heap_files.remove(name);

        // Delete heap file
        if Path::new(&metadata.heap_file_path).exists() {
            std::fs::remove_file(&metadata.heap_file_path)
                .map_err(|e| StorageError::Other(format!("Failed to delete heap file: {}", e)))?;
        }

        Ok(())
    }

    /// Check if a relation exists.
    pub fn relation_exists(&self, name: &str) -> bool {
        self.catalog.relation_exists(name)
    }

    /// Get relation metadata.
    pub fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        let metadata = self
            .catalog
            .get_relation(name)
            .map_err(Self::convert_catalog_error)?;

        Ok(RelationMetadata {
            name: name.to_string(),
            relation_type: metadata.relation_type.clone(),
        })
    }

    /// List all relations.
    pub fn list_relations(&self) -> Vec<String> {
        self.catalog.list_relations()
    }

    /// Get or open a heap file for a relation.
    ///
    /// Takes the catalog and heap-file cache as separate parameters (rather
    /// than `&mut self`) so callers can hold disjoint borrows of the version
    /// pool alongside the returned heap file.
    fn get_or_open_heap_file<'a>(
        catalog: &Catalog,
        heap_files: &'a mut HashMap<String, HeapFile<FileBlockDevice>>,
        name: &str,
    ) -> Result<&'a mut HeapFile<FileBlockDevice>, StorageError> {
        use std::collections::hash_map::Entry;
        match heap_files.entry(name.to_string()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let metadata = catalog
                    .get_relation(name)
                    .map_err(Self::convert_catalog_error)?;

                let heap_file =
                    HeapFile::open(&metadata.heap_file_path, metadata.relation_type.clone())
                        .map_err(Self::convert_heap_error)?;

                Ok(entry.insert(heap_file))
            }
        }
    }

    /// Scan a relation with visibility filtering (MVCC).
    pub fn scan_relation(
        &mut self,
        name: &str,
        snapshot: &TransactionSnapshot,
        pool: &crate::mvcc::TxnPool,
        committed_txns: &HashSet<TransactionId>,
    ) -> Result<Relation, StorageError> {
        // Get metadata
        let metadata = self
            .catalog
            .get_relation(name)
            .map_err(Self::convert_catalog_error)?;

        // Need relation type for building result
        let rel_type = metadata.relation_type.clone();

        // Get or open heap file. The heap-file cache and the version pool are
        // disjoint fields, so both borrows can be live at once.
        let heap_file = Self::get_or_open_heap_file(&self.catalog, &mut self.heap_files, name)?;
        let version_pool = &mut self.version_pool;

        // Scan with visibility filtering
        let tuples = heap_file
            .scan_visible(snapshot, pool, version_pool, committed_txns)
            .map_err(Self::convert_heap_error)?;

        // Build relation from visible tuples
        Relation::from_tuples(rel_type, tuples)
            .map_err(|e| StorageError::Other(format!("Failed to build relation: {}", e)))
    }

    /// Insert a tuple into a relation with versioning.
    pub fn insert_tuple(
        &mut self,
        name: &str,
        tuple: Tuple,
        txn_id: TransactionId,
    ) -> Result<(), StorageError> {
        let heap_file = Self::get_or_open_heap_file(&self.catalog, &mut self.heap_files, name)?;
        let version_pool = &mut self.version_pool;
        heap_file
            .insert_tuple_versioned(&tuple, txn_id, version_pool)
            .map_err(Self::convert_heap_error)?;
        Ok(())
    }

    /// Replace a relation entirely (store_relation).
    /// Used for bulk updates or initialization.
    pub fn store_relation(
        &mut self,
        name: &str,
        relation: &Relation,
        txn_id: TransactionId,
    ) -> Result<(), StorageError> {
        // Get metadata
        let metadata = self
            .catalog
            .get_relation(name)
            .map_err(Self::convert_catalog_error)?;

        // Build the replacement in a temp file and rename it over the old
        // heap atomically: if any insert fails (pool exhaustion, device
        // error, oversized tuple) the old heap is untouched and the temp
        // file is removed, so a failed store never destroys the relation.
        // The temp name appends a suffix in the same directory, so the
        // rename stays on one filesystem (and is atomic on POSIX).
        let mut tmp_path = metadata.heap_file_path.clone();
        tmp_path.as_mut_os_string().push(".tmp");
        // A stale temp file means a previous store crashed mid-replace;
        // the old heap is still canonical, so dropping the stale temp is
        // safe. Inspect the error kind directly instead of a Path::exists()
        // re-check (avoids the TOCTOU race and an extra syscall).
        if let Err(e) = std::fs::remove_file(&tmp_path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(StorageError::Other(format!(
                "Failed to remove stale store temp file: {e}"
            )));
        }

        // Create the replacement heap in the temp file.
        let mut new_heap_file = HeapFile::create(&tmp_path, metadata.relation_type.clone())
            .map_err(Self::convert_heap_error)?;

        // Insert all tuples with MVCC versioning. Any failure removes the
        // temp file and leaves the old heap (and its cache entry) intact.
        let insert_result: Result<(), StorageError> = (|| {
            for tuple in relation.tuples() {
                new_heap_file
                    .insert_tuple_versioned(tuple, txn_id, &mut self.version_pool)
                    .map_err(Self::convert_heap_error)?;
            }
            Ok(())
        })();
        if let Err(error) = insert_result {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(error);
        }

        // Durability before publish: flush the replacement's contents to
        // the device so a crash after the rename cannot surface a heap
        // whose pages never reached stable storage. A sync failure keeps
        // the old heap canonical and removes the temp file.
        if let Err(error) = new_heap_file.sync().map_err(Self::convert_heap_error) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(error);
        }

        // Atomic publish: the rename swaps the replacement in all at once.
        // Only now is the old heap dropped and its cache entry replaced.
        // Durability notes: the temp file lives in the same directory, so
        // the rename stays on one filesystem. Atomic replacement is a
        // POSIX guarantee (rename(2)); on Windows `rename` fails when the
        // destination exists, so the store fails typed with the old heap
        // intact rather than replacing non-atomically. The rename's own
        // directory entry is not fsynced — a crash in that narrow window
        // could leave the old name visible after reopen; the WAL still
        // records the committed transaction, so no committed data is
        // silently lost, only the bulk replace may need re-running.
        if let Err(e) = std::fs::rename(&tmp_path, &metadata.heap_file_path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(StorageError::Other(format!(
                "Failed to publish replaced heap file: {e}"
            )));
        }

        // Remove the stale cache entry and cache the replaced heap. The
        // handle was created on the temp path, but rename preserves the
        // inode, so it now refers to the canonical file.
        self.heap_files.remove(name);
        self.heap_files.insert(name.to_string(), new_heap_file);

        Ok(())
    }

    /// Flush all heap files to disk.
    pub fn flush_heap_files(&mut self) -> Result<(), StorageError> {
        for heap_file in self.heap_files.values_mut() {
            heap_file
                .sync()
                .map_err(|e| StorageError::Other(format!("Heap sync error: {}", e)))?;
        }
        Ok(())
    }

    /// Garbage collect old versions.
    ///
    /// A version is reclaimed when no past, present, or future snapshot can
    /// observe it: either its deleter's commit settled before `gc_lsn`
    /// (resolved through `txn_pool`), or its creator can never commit
    /// (neither committed nor live — e.g. an aborted transaction's
    /// inserts). See [`TxnPool::committed_before`] and [`TxnPool::is_live`].
    pub fn garbage_collect_versions(
        &mut self,
        gc_lsn: crate::wal::Lsn,
        committed_txns: &HashSet<TransactionId>,
        txn_pool: &TxnPool,
    ) -> Result<(), StorageError> {
        let version_pool = &mut self.version_pool;
        for heap_file in self.heap_files.values_mut() {
            heap_file
                .gc_remove_dead_versions(gc_lsn, committed_txns, txn_pool, version_pool)
                // Keep MVCC exhaustion typed (never downgraded to `Other`):
                // callers can match on it and retry after commit/abort.
                .map_err(Self::convert_heap_error)?;
        }
        Ok(())
    }

    /// Recycles all version-pool records claimed by `txn_id`.
    ///
    /// Called when a transaction commits or aborts: its versions are then
    /// committed (visible per MVCC rules) or dead (reclaimed by GC), so the
    /// pool records can back new writes. Infallible.
    pub fn release_versions_for_txn(&mut self, txn_id: TransactionId) {
        self.version_pool.release_for_txn(txn_id);
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

    fn dummy_snapshot() -> (crate::mvcc::TxnPool, TransactionSnapshot) {
        let mut pool = crate::mvcc::TxnPool::new(4);
        let snapshot = pool
            .begin(
                TransactionId::new(1),
                crate::wal::Lsn::new(0),
                TransactionId::new(u64::MAX),
            )
            .unwrap();
        (pool, snapshot)
    }

    fn committed_set() -> HashSet<TransactionId> {
        let mut s = HashSet::new();
        s.insert(TransactionId::new(1));
        s
    }

    #[test]
    fn test_create_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        manager.create_relation("TEST", test_rel_type()).unwrap();

        let txn_id = TransactionId::new(1);
        manager
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" }, txn_id)
            .unwrap();

        let (pool, snapshot) = dummy_snapshot();
        let relation = manager
            .scan_relation("TEST", &snapshot, &pool, &committed_set())
            .unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_drop_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        manager.create_relation("TEST", test_rel_type()).unwrap();
        assert!(manager.relation_exists("TEST"));

        manager.drop_relation("TEST").unwrap();
        assert!(!manager.relation_exists("TEST"));

        assert!(manager.drop_relation("TEST").is_err());
    }

    #[test]
    fn test_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        assert!(manager.create_relation("../evil", test_rel_type()).is_err());
        assert!(manager.create_relation("foo/bar", test_rel_type()).is_err());
        assert!(manager.create_relation("VALID", test_rel_type()).is_ok());
    }

    #[test]
    fn test_store_relation_replaces_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        manager.create_relation("TEST", test_rel_type()).unwrap();

        // Insert initial data
        let txn_id = TransactionId::new(1);
        manager
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" }, txn_id)
            .unwrap();

        // Create new relation with different data
        let mut new_relation = Relation::new(test_rel_type());
        new_relation
            .insert(tuple! { id: 100i64, name: "Charlie" })
            .unwrap();

        // Store should replace old data
        manager
            .store_relation("TEST", &new_relation, txn_id)
            .unwrap();

        let (pool, snapshot) = dummy_snapshot();
        let loaded = manager
            .scan_relation("TEST", &snapshot, &pool, &committed_set())
            .unwrap();
        assert_eq!(loaded.cardinality(), 1);
        assert!(loaded.contains(&tuple! { id: 100i64, name: "Charlie" }));
    }
}
