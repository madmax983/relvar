use crate::types::RelationType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Relation '{0}' not found")]
    RelationNotFound(String),
    #[error("Relation '{0}' already exists")]
    RelationExists(String),
}

/// Metadata for a stored relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationMetadata {
    pub name: String,
    pub relation_type: RelationType,
    pub heap_file_path: PathBuf,
}

/// System catalog stores metadata about all relations in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    relations: HashMap<String, RelationMetadata>,
}

impl Catalog {
    /// Create a new empty catalog
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
        }
    }

    /// Load catalog from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, CatalogError> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        if contents.is_empty() {
            return Ok(Self::new());
        }

        serde_json::from_str(&contents).map_err(|e| CatalogError::Serialization(e.to_string()))
    }

    /// Save catalog to a file
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

    /// Register a new relation
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
            name: name.clone(),
            relation_type,
            heap_file_path,
        };

        self.relations.insert(name, metadata);
        Ok(())
    }

    /// Get metadata for a relation
    pub fn get_relation(&self, name: &str) -> Result<&RelationMetadata, CatalogError> {
        self.relations
            .get(name)
            .ok_or_else(|| CatalogError::RelationNotFound(name.to_string()))
    }

    /// Check if a relation exists
    pub fn relation_exists(&self, name: &str) -> bool {
        self.relations.contains_key(name)
    }

    /// Drop a relation
    pub fn drop_relation(&mut self, name: &str) -> Result<RelationMetadata, CatalogError> {
        self.relations
            .remove(name)
            .ok_or_else(|| CatalogError::RelationNotFound(name.to_string()))
    }

    /// List all relation names
    pub fn list_relations(&self) -> Vec<String> {
        self.relations.keys().cloned().collect()
    }

    /// Get the number of relations
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
    use crate::types::{ScalarType, TupleType};
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
        assert_eq!(metadata.name, "employees");
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
}
