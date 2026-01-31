//! Persistent storage engine implementation using heap files and catalog.

use crate::storage::{Catalog, CatalogError, HeapError, HeapFile};
use relvar_core::storage_engine::{
    ConstraintMetadata, RelationMetadata, StorageEngine, StorageError,
};
use relvar_core::types::RelationType;
use relvar_core::values::{Relation, Tuple};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Snapshot for persistent transactions.
#[derive(Debug, Clone)]
pub struct PersistentSnapshot {
    /// Saved relations, keyed by name.
    pub saved_relations: HashMap<String, Relation>,
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
                    .map_err(Self::convert_heap_error)?;

            self.heap_files.insert(name.to_string(), heap_file);
        }

        Ok(self.heap_files.get_mut(name).unwrap())
    }

    /// Convert HeapError to StorageError.
    fn convert_heap_error(e: HeapError) -> StorageError {
        StorageError::Other(format!("Heap error: {}", e))
    }

    /// Validate that a relvar name is safe for use in file paths.
    ///
    /// Rejects names containing path separators or parent directory references
    /// to prevent path traversal attacks.
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
    type Snapshot = PersistentSnapshot;

    fn create_relation(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), StorageError> {
        // Validate relvar name for filesystem safety
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
                .map_err(Self::convert_heap_error)?;

        heap_file.load_relation().map_err(Self::convert_heap_error)
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
                .map_err(Self::convert_heap_error)?;

        // Insert all tuples
        for tuple in relation.tuples() {
            new_heap_file
                .insert_tuple(tuple)
                .map_err(Self::convert_heap_error)?;
        }

        // Cache the new heap file
        self.heap_files.insert(name.to_string(), new_heap_file);

        Ok(())
    }

    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        let heap_file = self.get_or_open_heap_file(name)?;
        heap_file
            .insert_tuple(&tuple)
            .map_err(Self::convert_heap_error)
    }

    fn save_constraints(
        &mut self,
        relation_name: &str,
        constraints: ConstraintMetadata,
    ) -> Result<(), StorageError> {
        self.catalog
            .update_constraints(relation_name, constraints)
            .map_err(|e| match e {
                CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
                CatalogError::Io(io_err) => {
                    StorageError::Other(format!("Catalog I/O error: {}", io_err))
                }
                CatalogError::Serialization(s) => {
                    StorageError::Other(format!("Catalog error: {}", s))
                }
                CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
            })?;

        self.save_catalog()?;
        Ok(())
    }

    fn load_constraints(&self, relation_name: &str) -> Result<ConstraintMetadata, StorageError> {
        let metadata = self
            .catalog
            .get_relation(relation_name)
            .map_err(|e| match e {
                CatalogError::RelationNotFound(r) => StorageError::RelationNotFound(r),
                CatalogError::Io(io_err) => {
                    StorageError::Other(format!("Catalog I/O error: {}", io_err))
                }
                CatalogError::Serialization(s) => {
                    StorageError::Other(format!("Catalog error: {}", s))
                }
                CatalogError::RelationExists(r) => StorageError::RelationAlreadyExists(r),
            })?;

        Ok(metadata.constraints.clone())
    }

    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError> {
        // Save current state of all relations
        let mut saved_relations = HashMap::new();

        for name in self.catalog.list_relations() {
            let relation = self.load_relation(&name)?;
            saved_relations.insert(name, relation);
        }

        Ok(PersistentSnapshot { saved_relations })
    }

    fn rollback_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
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

    #[test]
    fn test_constraint_persistence() {
        use relvar_core::constraints::{KeyConstraints, PrimaryKey};
        use relvar_core::database::{Database, DatabaseError};

        let temp_dir = TempDir::new().unwrap();

        // 1. Create DB, add relation and constraint
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let mut db = Database::open(engine).unwrap();

            db.create_relvar("USERS", test_rel_type()).unwrap();

            let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
            let constraints = KeyConstraints::new().with_primary_key(pk);
            db.set_key_constraints("USERS", constraints).unwrap();

            db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
        }

        // 2. Re-open DB and verify constraint
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let mut db = Database::open(engine).unwrap();

            // Insert duplicate PK
            let result = db.insert("USERS", tuple! { id: 1i64, name: "Bob" });
            assert!(result.is_err());
            assert!(matches!(result, Err(DatabaseError::PrimaryKeyViolation)));
        }
    }

    #[test]
    fn test_dynamic_constraint_persistence() {
        use relvar_core::constraints::expression::{ConstraintExpression, ValueOrRef};
        use relvar_core::constraints::{CheckConstraint, CheckConstraints};
        use relvar_core::database::{Database, DatabaseError};
        use relvar_core::values::ScalarValue;

        let temp_dir = TempDir::new().unwrap();

        // 1. Create DB with dynamic and expression constraints
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let mut db = Database::open(engine).unwrap();

            db.create_relvar("TEST", test_rel_type()).unwrap();

            // Dynamic constraint: fail if id == 100 (should be dropped on save)
            let dynamic = CheckConstraint::from_closure("dynamic", "desc", |t| {
                if let Some(ScalarValue::Int(id)) = t.get("id") {
                    *id != 100
                } else {
                    true
                }
            });

            // Expression constraint: fail if id < 0 (should be persisted)
            let expr = CheckConstraint::from_expression(
                "expr",
                "desc",
                ConstraintExpression::Ge("id".to_string(), ValueOrRef::Value(ScalarValue::Int(0))),
            );

            let constraints = CheckConstraints::new()
                .with_constraint(dynamic)
                .with_constraint(expr);

            db.set_check_constraints("TEST", constraints).unwrap();

            // Verify constraints active in memory
            assert!(
                db.insert("TEST", tuple! { id: 100i64, name: "Bad" })
                    .is_err()
            );
            assert!(
                db.insert("TEST", tuple! { id: -1i64, name: "Bad" })
                    .is_err()
            );
        }

        // 2. Re-open and verify
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            let mut db = Database::open(engine).unwrap();

            // Dynamic constraint should be gone: id=100 should succeed
            assert!(db.insert("TEST", tuple! { id: 100i64, name: "Ok" }).is_ok());

            // Expression constraint should persist: id=-1 should fail
            let result = db.insert("TEST", tuple! { id: -1i64, name: "Bad" });
            assert!(result.is_err());
            assert!(matches!(
                result,
                Err(DatabaseError::CheckConstraintViolation(_))
            ));
        }
    }
}
