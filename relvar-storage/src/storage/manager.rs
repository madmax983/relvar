//! Storage Manager for handling physical storage operations.
//!
//! This module encapsulates the management of the catalog and heap files,
//! providing a cleaner interface for the PersistentEngine.

use crate::mvcc::TransactionSnapshot;
use crate::storage::{Catalog, CatalogError, HeapError, HeapFile};
use crate::wal::{TransactionId, UncommittedInsert};
use relvar_core::storage_engine::{RelationMetadata, StorageError};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Manages physical storage components (Catalog and HeapFiles).
pub struct StorageManager {
    /// Base directory for database files.
    db_path: PathBuf,
    /// Path to the catalog file.
    catalog_path: PathBuf,
    /// System catalog containing relation metadata.
    catalog: Catalog,
    /// Open heap files, keyed by relation name.
    heap_files: HashMap<String, HeapFile>,
}

impl StorageManager {
    /// Open or create the storage manager at the specified path.
    pub fn open(db_path: PathBuf) -> Result<Self, StorageError> {
        let catalog_path = db_path.join("catalog.json");

        Self::ensure_db_directory(&db_path)?;

        let catalog = Self::load_catalog_from_disk(&catalog_path)?;

        Ok(Self {
            db_path,
            catalog_path,
            catalog,
            heap_files: HashMap::new(),
        })
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

        let heap_file_path = self.db_path.join(format!("{}.heap", name));

        let heap_file = HeapFile::create(&heap_file_path, relation_type.clone())
            .map_err(Self::convert_heap_error)?;

        self.catalog
            .create_relation(name.to_string(), relation_type, heap_file_path.clone())
            .map_err(Self::convert_catalog_error)?;

        self.save_catalog()?;

        self.heap_files.insert(name.to_string(), heap_file);

        Ok(())
    }

    /// Drop an existing relation.
    pub fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        let metadata = self
            .catalog
            .drop_relation(name)
            .map_err(Self::convert_catalog_error)?;

        self.save_catalog()?;
        self.heap_files.remove(name);

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

    /// List all relation names.
    pub fn list_relations(&self) -> Vec<String> {
        self.catalog.list_relations()
    }

    /// Load a relation with MVCC visibility filtering.
    ///
    /// Takes `&self` (immutable) to allow concurrent reads and match StorageEngine trait.
    /// This bypasses the heap file cache and opens a new file handle.
    pub fn load_relation(
        &self,
        name: &str,
        snapshot: &TransactionSnapshot,
        committed_txns: &HashSet<TransactionId>,
    ) -> Result<Relation, StorageError> {
        let metadata = self
            .catalog
            .get_relation(name)
            .map_err(Self::convert_catalog_error)?;

        // Bypassing cache since we only have &self
        let mut heap_file =
            HeapFile::open(&metadata.heap_file_path, metadata.relation_type.clone())
                .map_err(Self::convert_heap_error)?;

        let tuples = heap_file
            .scan_visible(snapshot, committed_txns)
            .map_err(Self::convert_heap_error)?;

        Relation::from_tuples(metadata.relation_type.clone(), tuples)
            .map_err(|e| StorageError::Other(format!("Failed to build relation: {}", e)))
    }

    /// Store a relation (replaces existing content).
    /// Used for bulk updates/deletes.
    pub fn store_relation(
        &mut self,
        name: &str,
        relation: &Relation,
        txn_id: TransactionId,
    ) -> Result<(), StorageError> {
        let metadata = self
            .catalog
            .get_relation(name)
            .map_err(Self::convert_catalog_error)?;

        #[allow(clippy::collapsible_if)]
        if let Err(e) = std::fs::remove_file(&metadata.heap_file_path) {
            if Path::new(&metadata.heap_file_path).exists() {
                return Err(StorageError::Other(format!(
                    "Failed to remove old heap file: {}",
                    e
                )));
            }
        }

        self.heap_files.remove(name);

        let mut new_heap_file =
            HeapFile::create(&metadata.heap_file_path, metadata.relation_type.clone())
                .map_err(Self::convert_heap_error)?;

        for tuple in relation.tuples() {
            new_heap_file
                .insert_tuple_versioned(tuple, txn_id)
                .map_err(Self::convert_heap_error)?;
        }

        self.heap_files.insert(name.to_string(), new_heap_file);

        Ok(())
    }

    /// Insert a tuple with versioning.
    pub fn insert_tuple_versioned(
        &mut self,
        name: &str,
        tuple: &Tuple,
        txn_id: TransactionId,
    ) -> Result<(), StorageError> {
        let heap_file = self.get_or_open_heap_file(name)?;
        heap_file
            .insert_tuple_versioned(tuple, txn_id)
            .map_err(Self::convert_heap_error)?;
        Ok(())
    }

    /// Flush all open heap files to disk.
    pub fn flush_all(&mut self) -> Result<(), StorageError> {
        for heap_file in self.heap_files.values_mut() {
            heap_file
                .sync()
                .map_err(|e| StorageError::Other(format!("Heap sync error: {}", e)))?;
        }
        Ok(())
    }

    /// Garbage collect old versions from heap files.
    pub fn garbage_collect_versions(
        &mut self,
        gc_lsn: crate::wal::Lsn,
        committed_txns: &HashSet<TransactionId>,
    ) -> Result<(), StorageError> {
        for heap_file in self.heap_files.values_mut() {
            crate::mvcc::gc::collect_garbage(heap_file, gc_lsn, committed_txns)
                .map_err(|e| StorageError::Other(format!("GC error: {}", e)))?;
        }
        Ok(())
    }

    /// Undoes uncommitted inserts identified during recovery.
    pub fn undo_uncommitted_inserts_with_committed(
        &mut self,
        uncommitted_inserts: Vec<UncommittedInsert>,
        current_lsn: crate::wal::Lsn,
        committed_txns: &HashSet<TransactionId>,
    ) -> Result<(), StorageError> {
        // Group by relation name
        let mut by_relation: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
        for insert in uncommitted_inserts {
            by_relation
                .entry(insert.relation_name)
                .or_default()
                .push(insert.tuple_data);
        }

        let snapshot = TransactionSnapshot::new(TransactionId::new(0), current_lsn, vec![]);

        for (relation_name, _) in by_relation {
            if !self.catalog.relation_exists(&relation_name) {
                continue;
            }

            // We load relation using &self.load_relation (which bypasses cache)
            // But we need to update cache eventually via store_relation
            let relation = self.load_relation(&relation_name, &snapshot, committed_txns)?;

            self.store_relation(&relation_name, &relation, TransactionId::new(0))?;
        }
        Ok(())
    }

    // Helper to get or open a heap file
    fn get_or_open_heap_file(&mut self, name: &str) -> Result<&mut HeapFile, StorageError> {
        if !self.heap_files.contains_key(name) {
            let metadata = self
                .catalog
                .get_relation(name)
                .map_err(Self::convert_catalog_error)?;

            let heap_file =
                HeapFile::open(&metadata.heap_file_path, metadata.relation_type.clone())
                    .map_err(Self::convert_heap_error)?;

            self.heap_files.insert(name.to_string(), heap_file);
        }

        Ok(self.heap_files.get_mut(name).unwrap())
    }

    // Helper methods
    fn ensure_db_directory(path: &Path) -> Result<(), StorageError> {
        if !path.exists() {
            std::fs::create_dir_all(path)
                .map_err(|e| StorageError::Other(format!("Failed to create directory: {}", e)))?;
        }
        Ok(())
    }

    fn load_catalog_from_disk(catalog_path: &Path) -> Result<Catalog, StorageError> {
        if catalog_path.exists() {
            Catalog::load(catalog_path).map_err(Self::convert_catalog_error)
        } else {
            Ok(Catalog::new())
        }
    }

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

    fn save_catalog(&self) -> Result<(), StorageError> {
        self.catalog
            .save(&self.catalog_path)
            .map_err(Self::convert_catalog_error)
    }

    fn convert_heap_error(e: HeapError) -> StorageError {
        StorageError::Other(format!("Heap error: {}", e))
    }

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
}
