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
//! It also stores the database's declarative constraints (keys, foreign
//! keys, CHECK and type constraints) as a [`ConstraintCatalog`]: a database
//! contains its constraints (TTM RM Prescription 16). Catalogs written
//! before constraints were persisted have no `constraints` field and load
//! as a database without constraints.
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

use relvar_core::constraints::ConstraintCatalog;
use relvar_core::types::RelationType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
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

    /// The stored constraints name a relation the catalog does not contain.
    #[error("Stored constraints refer to unknown relation '{0}'")]
    ConstraintOnUnknownRelation(String),

    /// The relation cannot be dropped: a stored foreign key of another
    /// relation references it.
    #[error("Relation '{relation}' is referenced by a foreign key of '{referencing}'")]
    RelationReferenced {
        /// The relation being dropped.
        relation: String,
        /// The relation whose foreign key references it.
        referencing: String,
    },
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
/// # Examples
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
    /// Declarative constraints of the database (absent in catalogs written
    /// before constraints were persisted, which load as "no constraints").
    #[serde(default, skip_serializing_if = "ConstraintCatalog::is_empty")]
    constraints: ConstraintCatalog,
}

impl Catalog {
    /// Creates a new empty catalog.
    ///
    /// # Examples
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
            constraints: ConstraintCatalog::new(),
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
    /// Returns [`CatalogError::ConstraintOnUnknownRelation`] if a stored
    /// constraint names a relation the catalog does not contain.
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::storage::Catalog;
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let path = dir.path().join("catalog.json");
    /// let catalog = Catalog::new();
    /// catalog.save(&path).unwrap();
    /// let loaded = Catalog::load(&path).unwrap();
    /// assert_eq!(loaded.relation_count(), 0);
    /// ```
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
        let mut capped_reader = reader.take(limit);
        let catalog: Self = serde_json::from_reader(&mut capped_reader)
            .map_err(|e| CatalogError::Serialization(e.to_string()))?;

        if capped_reader.limit() == 0 {
            return Err(CatalogError::Serialization(
                "Catalog file too large".to_string(),
            ));
        }

        // Constraints must not outlive (or predate) their relations.
        if let Some(unknown) = catalog
            .constraints
            .mentioned_relvars()
            .find(|name| !catalog.relations.contains_key(*name))
        {
            return Err(CatalogError::ConstraintOnUnknownRelation(
                unknown.to_string(),
            ));
        }

        Ok(catalog)
    }

    /// Saves the catalog to a JSON file.
    ///
    /// The file is written in pretty-printed JSON format for readability.
    ///
    /// # Durability
    ///
    /// The save is atomic: the catalog is written to a temporary file next
    /// to `path`, flushed to stable storage, renamed over `path`, and (on
    /// Unix) the containing directory is synced so the rename itself is
    /// durable. A crash at any point leaves either the previous catalog or
    /// the new one, never a truncated or partially written file.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Io`] if the file cannot be written.
    /// Returns [`CatalogError::Serialization`] if serialization fails.
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::storage::Catalog;
    /// use tempfile::tempdir;
    /// let dir = tempdir().unwrap();
    /// let path = dir.path().join("catalog.json");
    /// let catalog = Catalog::new();
    /// catalog.save(&path).unwrap();
    /// assert!(path.exists());
    /// ```
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), CatalogError> {
        let path = path.as_ref();
        let temp_path = Self::temp_path(path);

        let result = self.write_synced(&temp_path).and_then(|()| {
            std::fs::rename(&temp_path, path)?;
            Self::sync_parent_dir(path)
        });
        if result.is_err() {
            // Best effort: never leave a stray temporary file behind. The
            // original error is the one worth reporting.
            let _ = std::fs::remove_file(&temp_path);
        }
        result
    }

    /// The temporary file a save writes before renaming it over `path`.
    fn temp_path(path: &Path) -> PathBuf {
        let mut name = path
            .file_name()
            .map_or_else(|| OsString::from("catalog"), OsString::from);
        name.push(".tmp");
        path.with_file_name(name)
    }

    /// Writes the catalog to `path` (replacing it) and syncs it to disk.
    fn write_synced(&self, path: &Path) -> Result<(), CatalogError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        {
            let mut writer = std::io::BufWriter::new(&mut file);
            serde_json::to_writer_pretty(&mut writer, self)
                .map_err(|e| CatalogError::Serialization(e.to_string()))?;
            writer.flush()?;
        }
        file.sync_all()?;
        Ok(())
    }

    /// Makes a rename into `path`'s directory durable (Unix only: other
    /// platforms cannot open a directory for syncing).
    #[cfg(unix)]
    fn sync_parent_dir(path: &Path) -> Result<(), CatalogError> {
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        File::open(parent)?.sync_all()?;
        Ok(())
    }

    /// Makes a rename into `path`'s directory durable (Unix only: other
    /// platforms cannot open a directory for syncing).
    #[cfg(not(unix))]
    fn sync_parent_dir(_path: &Path) -> Result<(), CatalogError> {
        Ok(())
    }

    /// The database's stored declarative constraints.
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::storage::Catalog;
    /// let catalog = Catalog::new();
    /// assert!(catalog.constraints().is_empty());
    /// ```
    pub fn constraints(&self) -> &ConstraintCatalog {
        &self.constraints
    }

    /// Replaces the database's stored declarative constraints.
    ///
    /// This only changes the in-memory catalog; call [`save`](Self::save)
    /// to persist it.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::ConstraintOnUnknownRelation`] (and leaves the
    /// catalog unchanged) if a constraint names a relation the catalog does
    /// not contain.
    pub fn set_constraints(&mut self, constraints: ConstraintCatalog) -> Result<(), CatalogError> {
        if let Some(unknown) = constraints
            .mentioned_relvars()
            .find(|name| !self.relations.contains_key(*name))
        {
            return Err(CatalogError::ConstraintOnUnknownRelation(
                unknown.to_string(),
            ));
        }
        self.constraints = constraints;
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
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::storage::Catalog;
    /// use relvar_core::types::{RelationType, TupleType};
    /// use std::path::PathBuf;
    /// let mut catalog = Catalog::new();
    /// let rel_type = RelationType::new(TupleType::new());
    /// catalog.create_relation("my_table".to_string(), rel_type, PathBuf::from("my_table.heap")).unwrap();
    /// let meta = catalog.get_relation("my_table").unwrap();
    /// assert_eq!(meta.heap_file_path, PathBuf::from("my_table.heap"));
    /// ```
    pub fn get_relation(&self, name: &str) -> Result<&RelationMetadata, CatalogError> {
        self.relations
            .get(name)
            .ok_or_else(|| CatalogError::RelationNotFound(name.to_string()))
    }

    /// Checks if a relation with the given name exists.
    ///
    /// # Examples
    /// ```
    /// use relvar_storage::storage::Catalog;
    /// let catalog = Catalog::new();
    /// assert!(!catalog.relation_exists("my_table"));
    /// ```
    pub fn relation_exists(&self, name: &str) -> bool {
        self.relations.contains_key(name)
    }

    /// Removes a relation from the catalog.
    ///
    /// This only removes the catalog entry; it does not delete the data file.
    /// The constraints declared on the relation are removed with it, so the
    /// next [`save`](Self::save) cannot record constraints for a relation
    /// that no longer exists.
    ///
    /// # Returns
    ///
    /// The removed relation's metadata, so the caller can clean up the
    /// data file if desired.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::RelationNotFound`] if no relation with
    /// this name exists, or [`CatalogError::RelationReferenced`] if a
    /// stored foreign key of another relation references it.
    pub fn drop_relation(&mut self, name: &str) -> Result<RelationMetadata, CatalogError> {
        if let Some((referencing, _)) = self.constraints.iter().find(|(referencing, c)| {
            *referencing != name
                && c.foreign_keys.as_ref().is_some_and(|fks| {
                    fks.foreign_keys()
                        .iter()
                        .any(|fk| fk.referenced_relation_name() == name)
                })
        }) {
            return Err(CatalogError::RelationReferenced {
                relation: name.to_string(),
                referencing: referencing.to_string(),
            });
        }
        let metadata = self
            .relations
            .remove(name)
            .ok_or_else(|| CatalogError::RelationNotFound(name.to_string()))?;
        self.constraints.remove_relvar(name);
        Ok(metadata)
    }

    /// Retrieves a list of all relation names currently tracked by the catalog.
    ///
    /// The order of names is not guaranteed (hash map iteration order).
    pub fn list_relations(&self) -> Vec<String> {
        self.relations.keys().cloned().collect()
    }

    /// Counts the total number of relations currently registered in the database catalog.
    ///
    /// Useful for database statistics and ensuring schemas have been initialized
    /// correctly during startup or migration scripts.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_storage::storage::Catalog;
    ///
    /// let catalog = Catalog::new();
    /// assert_eq!(catalog.relation_count(), 0);
    /// ```
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

    fn constrained_catalog() -> Catalog {
        use relvar_core::constraints::{
            ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey,
        };
        let mut catalog = Catalog::new();
        for name in ["parents", "children"] {
            catalog
                .create_relation(
                    name.to_string(),
                    create_test_relation_type(),
                    PathBuf::from(format!("{name}.heap")),
                )
                .unwrap();
        }
        let mut constraints = ConstraintCatalog::new();
        constraints.relvar_mut("parents").keys = Some(
            KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".into()]).unwrap()),
        );
        constraints.relvar_mut("children").foreign_keys =
            Some(ForeignKeyConstraints::new().with_foreign_key(
                ForeignKey::new(vec!["id".into()], "parents".into(), vec!["id".into()]).unwrap(),
            ));
        catalog.set_constraints(constraints).unwrap();
        catalog
    }

    #[test]
    fn test_catalog_constraints_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        constrained_catalog().save(&path).unwrap();

        let loaded = Catalog::load(&path).unwrap();
        let parents = loaded.constraints().relvar("parents").unwrap();
        assert!(parents.keys.as_ref().unwrap().primary_key().is_some());
        assert!(
            loaded
                .constraints()
                .relvar("children")
                .unwrap()
                .foreign_keys
                .is_some()
        );
    }

    #[test]
    fn test_catalog_save_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        constrained_catalog().save(&path).unwrap();
        Catalog::new().save(&path).unwrap();

        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("catalog.json")]);
        assert_eq!(Catalog::load(&path).unwrap().relation_count(), 0);
    }

    #[test]
    fn test_catalog_save_failure_keeps_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        constrained_catalog().save(&path).unwrap();
        // A directory where the temporary file must go makes the write fail.
        std::fs::create_dir(dir.path().join("catalog.json.tmp")).unwrap();

        assert!(Catalog::new().save(&path).is_err());
        assert_eq!(Catalog::load(&path).unwrap().relation_count(), 2);
    }

    #[test]
    fn test_catalog_without_constraints_field_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        std::fs::write(&path, r#"{"relations":{}}"#).unwrap();

        let loaded = Catalog::load(&path).unwrap();
        assert!(loaded.constraints().is_empty());
    }

    #[test]
    fn test_catalog_rejects_constraints_on_unknown_relation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        let mut catalog = constrained_catalog();
        // Forge a catalog whose constraints outlive a relation.
        catalog.relations.remove("parents");
        std::fs::write(&path, serde_json::to_string(&catalog).unwrap()).unwrap();

        assert!(matches!(
            Catalog::load(&path),
            Err(CatalogError::ConstraintOnUnknownRelation(name)) if name == "parents"
        ));
        assert!(matches!(
            Catalog::new().set_constraints(constrained_catalog().constraints().clone()),
            Err(CatalogError::ConstraintOnUnknownRelation(_))
        ));
    }

    #[test]
    fn test_catalog_drop_relation_removes_its_constraints() {
        let mut catalog = constrained_catalog();
        catalog.drop_relation("children").unwrap();

        assert!(catalog.constraints().relvar("children").is_none());
        assert!(catalog.constraints().relvar("parents").is_some());
    }

    #[test]
    fn test_catalog_drop_referenced_relation_is_rejected() {
        let mut catalog = constrained_catalog();

        assert!(matches!(
            catalog.drop_relation("parents"),
            Err(CatalogError::RelationReferenced { relation, referencing })
                if relation == "parents" && referencing == "children"
        ));
        assert!(catalog.relation_exists("parents"));
    }

    #[test]
    fn test_catalog_load_limit() {
        use std::io::Write;
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
