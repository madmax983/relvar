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
        // Not reachable block covered in tests, because we don't have permission to write to /usr or such,
        // but wait, `test_manager_new_directory_creation_io_error` handles this.
        } else if !db_path.is_dir() {
            // Unreachable because we can't test it cleanly with `create_dir_all` failure logic in standard tests.
            // Oh actually, it's covered by `test_manager_new_directory_creation_error`! Wait, `test_manager_new_directory_creation_error` is failing because `StorageManager::new` returned Ok?
            // No, the test `test_manager_new_directory_creation_error` is passing now! Wait, if it passes, why is this not covered?
            // Ah, maybe the previous `return Err(...)` line wasn't covered.
            return Err(StorageError::Other("Path exists but is not a directory".to_string()));
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

    #[test]
    fn test_manager_validate_relvar_name_dot_dot() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        let result = manager.create_relation("..", test_rel_type());
        assert!(result.is_err());
        match result {
            Err(e) => {
                let msg = format!("{:?}", e);
                assert!(msg.contains("parent directory references"));
            }
            _ => panic!("Expected error"),
        }

        let result = manager.create_relation("a..b", test_rel_type());
        assert!(result.is_err());
        match result {
            Err(e) => {
                let msg = format!("{:?}", e);
                assert!(msg.contains("parent directory references"));
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_manager_validate_relvar_name_backslash() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        let result = manager.create_relation("A\\B", test_rel_type());
        assert!(result.is_err());
        match result {
            Err(e) => {
                let msg = format!("{:?}", e);
                assert!(msg.contains("cannot contain path separators"));
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_manager_validate_relvar_name_slash() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        let result = manager.create_relation("A/B", test_rel_type());
        assert!(result.is_err());
        match result {
            Err(e) => {
                let msg = format!("{:?}", e);
                assert!(msg.contains("cannot contain path separators"));
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_manager_convert_heap_error() {
        let err = StorageManager::convert_heap_error(HeapError::Serialization(
            "test io error".to_string(),
        ));
        assert!(matches!(err, StorageError::Other(_)));
    }

    #[test]
    fn test_manager_convert_catalog_error_io() {
        let err = StorageManager::convert_catalog_error(CatalogError::Io(std::io::Error::other(
            "test io error"
        )));
        assert!(matches!(err, StorageError::Other(_)));
    }

    #[test]
    fn test_manager_get_relation_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();
        manager.create_relation("A", test_rel_type()).unwrap();

        let meta = manager.get_relation_metadata("A").unwrap();
        assert_eq!(meta.name, "A");
    }
    #[test]
    fn test_manager_convert_catalog_error_not_found() {
        let result =
            StorageManager::convert_catalog_error(CatalogError::RelationNotFound("A".to_string()));
        assert!(matches!(result, StorageError::RelationNotFound(_)));
    }
    #[test]
    fn test_manager_convert_catalog_error() {
        let result =
            StorageManager::convert_catalog_error(CatalogError::RelationExists("A".to_string()));
        assert!(matches!(result, StorageError::RelationAlreadyExists(_)));

        let result =
            StorageManager::convert_catalog_error(CatalogError::Serialization("S".to_string()));
        assert!(matches!(result, StorageError::Other(_)));
    }

    #[test]
    fn test_manager_validate_relvar_name_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        let result = manager.create_relation("", test_rel_type());
        assert!(result.is_err());
        match result {
            Err(e) => {
                let msg = format!("{:?}", e);
                assert!(msg.contains("cannot be empty"));
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_manager_validate_relvar_name_dot() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        let result = manager.create_relation(".", test_rel_type());
        assert!(result.is_err());
        match result {
            Err(e) => {
                let msg = format!("{:?}", e);
                assert!(msg.contains("parent directory references"));
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_manager_store_relation_remove_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        manager
            .create_relation("TEST_REL", test_rel_type())
            .unwrap();

        // Create a directory where the heap file should be, to cause remove_file to fail
        let heap_path = temp_dir.path().join("TEST_REL.heap");
        let _ = std::fs::remove_file(&heap_path); // remove if exists
        std::fs::create_dir(&heap_path).unwrap(); // create directory instead

        let relation = Relation::new(test_rel_type());
        let txn_id = TransactionId::new(1);

        let result = manager.store_relation("TEST_REL", &relation, txn_id);
        assert!(result.is_err());

        // clean up
        std::fs::remove_dir(&heap_path).unwrap();
    }

    #[test]
    fn test_manager_drop_relation_remove_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();

        manager
            .create_relation("TEST_DROP", test_rel_type())
            .unwrap();

        let heap_path = temp_dir.path().join("TEST_DROP.heap");
        let _ = std::fs::remove_file(&heap_path);
        std::fs::create_dir(&heap_path).unwrap();

        let result = manager.drop_relation("TEST_DROP");
        assert!(result.is_err());

        std::fs::remove_dir(&heap_path).unwrap();
    }

    #[test]
    fn test_manager_new_directory_creation_io_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "test").unwrap();

        let db_path = file_path.join("some_dir");
        let result = StorageManager::new(&db_path);
        assert!(result.is_err());
        match result {
            Err(StorageError::Other(msg)) => {
                assert!(msg.contains("Failed to create directory"));
            }
            _ => panic!("Expected StorageError::Other"),
        }
    }
    #[test]
    fn test_manager_new_directory_creation_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "test").unwrap();
        let result = StorageManager::new(&file_path);
        assert!(
            result.is_err(),
            "Expected error when creating manager over existing file"
        );
    }
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
        TransactionSnapshot::new(TransactionId::new(1), crate::wal::Lsn::new(0), vec![])
    }

    fn committed_set() -> HashSet<TransactionId> {
        let mut s = HashSet::new();
        s.insert(TransactionId::new(1));
        s
    }

    #[test]
    fn test_manager_list_relations() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();
        manager.create_relation("A", test_rel_type()).unwrap();
        manager.create_relation("B", test_rel_type()).unwrap();
        let mut rels = manager.list_relations();
        rels.sort();
        assert_eq!(rels, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn test_manager_flush_heap_files() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();
        manager.create_relation("TEST", test_rel_type()).unwrap();
        let txn_id = TransactionId::new(1);
        manager
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" }, txn_id)
            .unwrap();

        let res = manager.flush_heap_files();
        assert!(res.is_ok());
    }

    #[test]
    fn test_manager_garbage_collect_versions() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path()).unwrap();
        manager.create_relation("TEST", test_rel_type()).unwrap();
        let txn_id = TransactionId::new(1);
        manager
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" }, txn_id)
            .unwrap();

        let mut committed = HashSet::new();
        committed.insert(txn_id);

        let res = manager.garbage_collect_versions(crate::wal::Lsn::new(100), &committed);
        assert!(res.is_ok());
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
