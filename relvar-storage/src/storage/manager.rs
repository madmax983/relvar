//! Storage Manager for physical data management.
//!
//! This module handles the physical storage layer operations, including:
//! - Catalog management (metadata)
//! - Heap file management (tuple storage)
//! - Garbage collection
//!
//! It abstracts the physical storage details from the transaction management layer.

use crate::mvcc::TransactionSnapshot;
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
    heap_files: HashMap<String, HeapFile>,
}

impl StorageManager {
    /// Create a new StorageManager instance.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
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
    fn convert_heap_error(e: HeapError) -> StorageError {
        StorageError::Other(format!("Heap error: {}", e))
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
    fn get_or_open_heap_file(&mut self, name: &str) -> Result<&mut HeapFile, StorageError> {
        use std::collections::hash_map::Entry;
        match self.heap_files.entry(name.to_string()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let metadata = self
                    .catalog
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
        committed_txns: &HashSet<TransactionId>,
    ) -> Result<Relation, StorageError> {
        // Get metadata
        let metadata = self
            .catalog
            .get_relation(name)
            .map_err(Self::convert_catalog_error)?;

        // Need relation type for building result
        let rel_type = metadata.relation_type.clone();

        // Get or open heap file
        let heap_file = self.get_or_open_heap_file(name)?;

        // Scan with visibility filtering
        let tuples = heap_file
            .scan_visible(snapshot, committed_txns)
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
        let heap_file = self.get_or_open_heap_file(name)?;
        heap_file
            .insert_tuple_versioned(&tuple, txn_id)
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

        // Remove old heap file and create new one
        if let Err(e) = std::fs::remove_file(&metadata.heap_file_path) {
            // Only error if file exists but can't be removed
            if Path::new(&metadata.heap_file_path).exists() {
                return Err(StorageError::Other(format!(
                    "Failed to remove old heap file: {}",
                    e
                )));
            }
        }

        // Remove from cache
        self.heap_files.remove(name);

        // Create new heap file
        let mut new_heap_file =
            HeapFile::create(&metadata.heap_file_path, metadata.relation_type.clone())
                .map_err(Self::convert_heap_error)?;

        // Insert all tuples with MVCC versioning
        for tuple in relation.tuples() {
            new_heap_file
                .insert_tuple_versioned(tuple, txn_id)
                .map_err(Self::convert_heap_error)?;
        }

        // Cache the new heap file
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
    pub fn garbage_collect_versions(
        &mut self,
        gc_lsn: crate::wal::Lsn,
        committed_txns: &HashSet<TransactionId>,
    ) -> Result<(), StorageError> {
        for heap_file in self.heap_files.values_mut() {
            heap_file
                .gc_remove_dead_versions(gc_lsn, committed_txns)
                .map_err(|e| StorageError::Other(format!("GC error: {}", e)))?;
        }
        Ok(())
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

    fn dummy_snapshot() -> TransactionSnapshot {
        TransactionSnapshot::new(TransactionId::new(1), crate::wal::Lsn::new(0), std::collections::HashSet::from([]))
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

        let relation = manager
            .scan_relation("TEST", &dummy_snapshot(), &committed_set())
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

        let loaded = manager
            .scan_relation("TEST", &dummy_snapshot(), &committed_set())
            .unwrap();
        assert_eq!(loaded.cardinality(), 1);
        assert!(loaded.contains(&tuple! { id: 100i64, name: "Charlie" }));
    }
}
