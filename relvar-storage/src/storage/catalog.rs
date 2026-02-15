//! System catalog for database metadata management.
//!
//! The catalog stores metadata about all relations in the database, including
//! their names, types (headings), and storage locations. It serves as the
//! single source of truth for database schema information.
//!
//! # Metadata Storage
//!
//! For each relation, the catalog stores:
//! - **Name** - The unique identifier for the relation
//! - **RelationType** - The heading (attribute names and types)
//! - **Heap file path** - Location of the data file on disk
//!
//! # Example
//!
//! ```no_run
//! use relvar_storage::storage::Catalog;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use std::path::PathBuf;
//!
//! let mut catalog = Catalog::new();
//!
//! // Define relation type
//! let heading = TupleType::new()
//!     .with_attribute("id".to_string(), ScalarType::Int)
//!     .with_attribute("name".to_string(), ScalarType::String);
//! let rel_type = RelationType::new(heading);
//!
//! // Register relation in catalog
//! catalog.create_relation(
//!     "employees".to_string(),
//!     rel_type.clone(),
//!     PathBuf::from("employees.heap"),
//! ).unwrap();
//!
//! // Look up relation metadata
//! let metadata = catalog.get_relation("employees").unwrap();
//! assert_eq!(metadata.relation_type, rel_type);
//! ```

use relvar_core::types::RelationType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during catalog operations.
#[derive(Debug, Error)]
pub enum CatalogError {
    /// An I/O error occurred while reading or writing the catalog.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization or deserialization of the catalog failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The requested relation was not found in the catalog.
    #[error("Relation '{0}' not found")]
    RelationNotFound(String),

    /// A relation with this name already exists.
    #[error("Relation '{0}' already exists")]
    RelationExists(String),
}

const MAX_CATALOG_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

/// Metadata for a stored relation (relvar).
///
/// Contains all the information needed to locate and interpret a relation's
/// data on disk.
///
/// # Fields
///
/// - `relation_type` - The type (heading) defining attribute names and types
/// - `heap_file_path` - Path to the heap file containing the relation's tuples
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationMetadata {
    /// The relation's type (heading).
    pub relation_type: RelationType,
    /// Path to the heap file storing the relation's data.
    pub heap_file_path: PathBuf,
}

/// The system catalog storing metadata for all relations in the database.
///
/// The catalog is the central registry for relation metadata. It maintains
/// a mapping from relation names to their metadata (type and storage location).
///
/// # Persistence
///
/// The catalog can be saved to and loaded from a JSON file for persistence
/// across database restarts.
///
/// # Example
///
/// ```no_run
/// use relvar_storage::storage::Catalog;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
/// use std::path::PathBuf;
///
/// // Create and populate catalog
/// let mut catalog = Catalog::new();
///
/// let emp_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id".to_string(), ScalarType::Int)
///         .with_attribute("name".to_string(), ScalarType::String)
/// );
///
/// catalog.create_relation(
///     "employees".to_string(),
///     emp_type,
///     PathBuf::from("data/employees.heap"),
/// ).unwrap();
///
/// // Save to disk
/// catalog.save("catalog.json").unwrap();
///
/// // Later: load from disk
/// let loaded = Catalog::load("catalog.json").unwrap();
/// assert!(loaded.relation_exists("employees"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    /// Map from relation name to metadata.
    relations: HashMap<String, RelationMetadata>,
}

impl Catalog {
    /// Creates a new empty catalog.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_storage::storage::Catalog;
    ///
    /// let catalog = Catalog::new();
    /// assert_eq!(catalog.relation_count(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
        }
    }

    /// Loads a catalog from a JSON file.
    ///
    /// If the file is empty, returns an empty catalog.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Io`] if the file cannot be read.
    /// Returns [`CatalogError::Serialization`] if the JSON is invalid.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, CatalogError> {
        Self::load_with_limit(path, MAX_CATALOG_SIZE)
    }

    fn load_with_limit<P: AsRef<Path>>(path: P, limit: u64) -> Result<Self, CatalogError> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();

        if len > limit {
            return Err(CatalogError::Serialization(
                "Catalog file too large".to_string(),
            ));
        }

        if len == 0 {
            return Ok(Self::new());
        }

        let reader = BufReader::new(file);
        serde_json::from_reader(reader.take(limit))
            .map_err(|e| CatalogError::Serialization(e.to_string()))
    }

    /// Saves the catalog to a JSON file.
    ///
    /// The file is written in pretty-printed JSON format for readability.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Io`] if the file cannot be written.
    /// Returns [`CatalogError::Serialization`] if serialization fails.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), CatalogError> {
        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| CatalogError::Serialization(e.to_string()))?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.write_all(contents.as_bytes())?;
        file.sync_all()?;

        Ok(())
    }

    /// Registers a new relation in the catalog.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the relation
    /// * `relation_type` - The relation's type (heading)
    /// * `heap_file_path` - Path where the relation's data will be stored
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::RelationExists`] if a relation with this
    /// name already exists.
    pub fn create_relation(
        &mut self,
        name: String,
        relation_type: RelationType,
        heap_file_path: PathBuf,
    ) -> Result<(), CatalogError> {
        if self.relations.contains_key(&name) {
            return Err(CatalogError::RelationExists(name));
        }

        let metadata = RelationMetadata {
            relation_type,
            heap_file_path,
        };

        self.relations.insert(name, metadata);
        Ok(())
    }

    /// Retrieves metadata for a relation by name.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::RelationNotFound`] if no relation with
    /// this name exists.
    pub fn get_relation(&self, name: &str) -> Result<&RelationMetadata, CatalogError> {
        self.relations
            .get(name)
            .ok_or_else(|| CatalogError::RelationNotFound(name.to_string()))
    }

    /// Checks if a relation with the given name exists.
    pub fn relation_exists(&self, name: &str) -> bool {
        self.relations.contains_key(name)
    }

    /// Removes a relation from the catalog.
    ///
    /// This only removes the catalog entry; it does not delete the data file.
    ///
    /// # Returns
    ///
    /// The removed relation's metadata, so the caller can clean up the
    /// data file if desired.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::RelationNotFound`] if no relation with
    /// this name exists.
    pub fn drop_relation(&mut self, name: &str) -> Result<RelationMetadata, CatalogError> {
        self.relations
            .remove(name)
            .ok_or_else(|| CatalogError::RelationNotFound(name.to_string()))
    }

    /// Returns a list of all relation names in the catalog.
    ///
    /// The order of names is not guaranteed (hash map iteration order).
    pub fn list_relations(&self) -> Vec<String> {
        self.relations.keys().cloned().collect()
    }

    /// Returns the number of relations in the catalog.
    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::types::{ScalarType, TupleType};
    use tempfile::NamedTempFile;

    fn create_test_relation_type() -> RelationType {
        let heading = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        RelationType::new(heading)
    }

    #[test]
    fn test_catalog_create_relation() {
        let mut catalog = Catalog::new();
        let rel_type = create_test_relation_type();

        catalog
            .create_relation(
                "employees".to_string(),
                rel_type,
                PathBuf::from("employees.heap"),
            )
            .unwrap();

        assert!(catalog.relation_exists("employees"));
        assert_eq!(catalog.relation_count(), 1);
    }

    #[test]
    fn test_catalog_get_relation() {
        let mut catalog = Catalog::new();
        let rel_type = create_test_relation_type();

        catalog
            .create_relation(
                "employees".to_string(),
                rel_type.clone(),
                PathBuf::from("employees.heap"),
            )
            .unwrap();

        let metadata = catalog.get_relation("employees").unwrap();
        assert_eq!(metadata.relation_type, rel_type);
    }

    #[test]
    fn test_catalog_relation_exists() {
        let mut catalog = Catalog::new();
        let rel_type = create_test_relation_type();

        assert!(!catalog.relation_exists("employees"));

        catalog
            .create_relation(
                "employees".to_string(),
                rel_type,
                PathBuf::from("employees.heap"),
            )
            .unwrap();

        assert!(catalog.relation_exists("employees"));
    }

    #[test]
    fn test_catalog_duplicate_relation() {
        let mut catalog = Catalog::new();
        let rel_type = create_test_relation_type();

        catalog
            .create_relation(
                "employees".to_string(),
                rel_type.clone(),
                PathBuf::from("employees.heap"),
            )
            .unwrap();

        let result = catalog.create_relation(
            "employees".to_string(),
            rel_type,
            PathBuf::from("employees2.heap"),
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CatalogError::RelationExists(_)
        ));
    }

    #[test]
    fn test_catalog_drop_relation() {
        let mut catalog = Catalog::new();
        let rel_type = create_test_relation_type();

        catalog
            .create_relation(
                "employees".to_string(),
                rel_type,
                PathBuf::from("employees.heap"),
            )
            .unwrap();

        assert_eq!(catalog.relation_count(), 1);

        catalog.drop_relation("employees").unwrap();

        assert_eq!(catalog.relation_count(), 0);
        assert!(!catalog.relation_exists("employees"));
    }

    #[test]
    fn test_catalog_list_relations() {
        let mut catalog = Catalog::new();
        let rel_type = create_test_relation_type();

        catalog
            .create_relation(
                "employees".to_string(),
                rel_type.clone(),
                PathBuf::from("employees.heap"),
            )
            .unwrap();

        catalog
            .create_relation(
                "departments".to_string(),
                rel_type,
                PathBuf::from("departments.heap"),
            )
            .unwrap();

        let mut relations = catalog.list_relations();
        relations.sort();

        assert_eq!(relations, vec!["departments", "employees"]);
    }

    #[test]
    fn test_catalog_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let rel_type = create_test_relation_type();

        // Create and save catalog
        {
            let mut catalog = Catalog::new();
            catalog
                .create_relation(
                    "employees".to_string(),
                    rel_type.clone(),
                    PathBuf::from("employees.heap"),
                )
                .unwrap();

            catalog
                .create_relation(
                    "departments".to_string(),
                    rel_type.clone(),
                    PathBuf::from("departments.heap"),
                )
                .unwrap();

            catalog.save(path).unwrap();
        }

        // Load catalog
        {
            let catalog = Catalog::load(path).unwrap();
            assert_eq!(catalog.relation_count(), 2);
            assert!(catalog.relation_exists("employees"));
            assert!(catalog.relation_exists("departments"));
        }
    }

    #[test]
    fn test_catalog_load_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create empty file
        File::create(path).unwrap();

        let catalog = Catalog::load(path).unwrap();
        assert_eq!(catalog.relation_count(), 0);
    }

    #[test]
    fn test_catalog_get_nonexistent_relation() {
        let catalog = Catalog::new();
        let result = catalog.get_relation("nonexistent");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CatalogError::RelationNotFound(_)
        ));
    }

    #[test]
    fn test_catalog_load_limit() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write 20 bytes
        {
            let mut file = File::create(path).unwrap();
            file.write_all(&[b'a'; 20]).unwrap();
        }

        // Try to load with limit 10
        let result = Catalog::load_with_limit(path, 10);
        assert!(result.is_err());
        match result.unwrap_err() {
            CatalogError::Serialization(msg) => {
                assert_eq!(msg, "Catalog file too large");
            }
            err => panic!("Expected Serialization error, got {:?}", err),
        }
    }
}
