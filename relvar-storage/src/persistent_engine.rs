//! Persistent storage engine implementation using heap files and catalog.

use crate::storage::{Catalog, CatalogError, HeapError, HeapFile};
use relvar_core::storage_engine::{
    RelationMetadata, StorageEngine, StorageError, TransactionSnapshot,
};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    /// Base directory for database files.
    db_path: PathBuf,
    /// Path to the catalog file.
    catalog_path: PathBuf,
    /// System catalog containing relation metadata.
    catalog: Catalog,
    /// Open heap files, keyed by relation name.
    heap_files: HashMap<String, HeapFile>,
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
        let catalog_path = db_path.join("catalog.json");

        // Create directory if it doesn't exist
        if !db_path.exists() {
            std::fs::create_dir_all(&db_path)
                .map_err(|e| StorageError::Other(format!("Failed to create directory: {}", e)))?;
        }

        // Load or create catalog
        let catalog = if catalog_path.exists() {
            Catalog::load(&catalog_path).map_err(|e| match e {
                CatalogError::Io(io_err) => {
                    StorageError::Other(format!("Catalog I/O error: {}", io_err))
                }
                CatalogError::Serialization(s) => {
                    StorageError::Other(format!("Catalog serialization error: {}", s))
                }
                CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
                CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
            })?
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

    /// Get or open a heap file for a relation.
    fn get_or_open_heap_file(&mut self, name: &str) -> Result<&mut HeapFile, StorageError> {
        if !self.heap_files.contains_key(name) {
            let metadata = self.catalog.get_relation(name).map_err(|e| match e {
                CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
                CatalogError::Io(io_err) => {
                    StorageError::Other(format!("Catalog I/O error: {}", io_err))
                }
                CatalogError::Serialization(s) => {
                    StorageError::Other(format!("Catalog error: {}", s))
                }
                CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
            })?;

            let heap_file =
                HeapFile::open(&metadata.heap_file_path, metadata.relation_type.clone())
                    .map_err(|e| Self::convert_heap_error(e))?;

            self.heap_files.insert(name.to_string(), heap_file);
        }

        Ok(self.heap_files.get_mut(name).unwrap())
    }

    /// Convert HeapError to StorageError.
    fn convert_heap_error(e: HeapError) -> StorageError {
        StorageError::Other(format!("Heap error: {}", e))
    }

    /// Save the catalog to disk.
    fn save_catalog(&self) -> Result<(), StorageError> {
        self.catalog.save(&self.catalog_path).map_err(|e| match e {
            CatalogError::Io(io_err) => {
                StorageError::Other(format!("Catalog I/O error: {}", io_err))
            }
            CatalogError::Serialization(s) => {
                StorageError::Other(format!("Catalog serialization error: {}", s))
            }
            CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
        })
    }
}

impl StorageEngine for PersistentEngine {
    fn create_relation(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), StorageError> {
        if self.catalog.relation_exists(name) {
            return Err(StorageError::RelationAlreadyExists(name.to_string()));
        }

        // Create heap file path
        let heap_file_path = self.db_path.join(format!("{}.heap", name));

        // Create the heap file
        let heap_file = HeapFile::create(&heap_file_path, relation_type.clone())
            .map_err(|e| Self::convert_heap_error(e))?;

        // Add to catalog
        self.catalog
            .create_relation(name.to_string(), relation_type, heap_file_path.clone())
            .map_err(|e| match e {
                CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
                CatalogError::Io(io_err) => {
                    StorageError::Other(format!("Catalog I/O error: {}", io_err))
                }
                CatalogError::Serialization(s) => {
                    StorageError::Other(format!("Catalog error: {}", s))
                }
                CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            })?;

        // Save catalog
        self.save_catalog()?;

        // Cache the heap file
        self.heap_files.insert(name.to_string(), heap_file);

        Ok(())
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        // Remove from catalog
        let metadata = self.catalog.drop_relation(name).map_err(|e| match e {
            CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            CatalogError::Io(io_err) => {
                StorageError::Other(format!("Catalog I/O error: {}", io_err))
            }
            CatalogError::Serialization(s) => StorageError::Other(format!("Catalog error: {}", s)),
            CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
        })?;

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

    fn relation_exists(&self, name: &str) -> bool {
        self.catalog.relation_exists(name)
    }

    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        let metadata = self.catalog.get_relation(name).map_err(|e| match e {
            CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            CatalogError::Io(io_err) => {
                StorageError::Other(format!("Catalog I/O error: {}", io_err))
            }
            CatalogError::Serialization(s) => StorageError::Other(format!("Catalog error: {}", s)),
            CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
        })?;

        Ok(RelationMetadata {
            name: name.to_string(),
            relation_type: metadata.relation_type.clone(),
        })
    }

    fn list_relations(&self) -> Vec<String> {
        self.catalog.list_relations()
    }

    fn load_relation(&self, name: &str) -> Result<Relation, StorageError> {
        // Need to open the heap file to load
        let metadata = self.catalog.get_relation(name).map_err(|e| match e {
            CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            CatalogError::Io(io_err) => {
                StorageError::Other(format!("Catalog I/O error: {}", io_err))
            }
            CatalogError::Serialization(s) => StorageError::Other(format!("Catalog error: {}", s)),
            CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
        })?;

        let mut heap_file =
            HeapFile::open(&metadata.heap_file_path, metadata.relation_type.clone())
                .map_err(|e| Self::convert_heap_error(e))?;

        heap_file
            .load_relation()
            .map_err(|e| Self::convert_heap_error(e))
    }

    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        // Get metadata
        let metadata = self.catalog.get_relation(name).map_err(|e| match e {
            CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
            CatalogError::Io(io_err) => {
                StorageError::Other(format!("Catalog I/O error: {}", io_err))
            }
            CatalogError::Serialization(s) => StorageError::Other(format!("Catalog error: {}", s)),
            CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
        })?;

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
                .map_err(|e| Self::convert_heap_error(e))?;

        // Insert all tuples
        for tuple in relation.tuples() {
            new_heap_file
                .insert_tuple(tuple)
                .map_err(|e| Self::convert_heap_error(e))?;
        }

        // Cache the new heap file
        self.heap_files.insert(name.to_string(), new_heap_file);

        Ok(())
    }

    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        let heap_file = self.get_or_open_heap_file(name)?;
        heap_file
            .insert_tuple(&tuple)
            .map_err(|e| Self::convert_heap_error(e))
    }

    fn begin_transaction(&mut self) -> Result<TransactionSnapshot, StorageError> {
        // Save current state of all relations
        let mut saved_relations = HashMap::new();

        for name in self.catalog.list_relations() {
            let relation = self.load_relation(&name)?;
            saved_relations.insert(name, relation);
        }

        Ok(TransactionSnapshot { saved_relations })
    }

    fn rollback_transaction(&mut self, snapshot: TransactionSnapshot) -> Result<(), StorageError> {
        // Restore all relations from snapshot
        for (name, relation) in snapshot.saved_relations {
            self.store_relation(&name, &relation)?;
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
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
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
}
