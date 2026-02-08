use crate::storage::{Catalog, CatalogError};
use relvar_core::storage_engine::{RelationMetadata, StorageError};
use relvar_core::types::RelationType;
use std::path::PathBuf;

/// Manages the database catalog, including persistence.
pub struct CatalogManager {
    /// Path to the catalog file on disk.
    catalog_path: PathBuf,
    /// The in-memory catalog structure.
    catalog: Catalog,
}

impl CatalogManager {
    /// Load an existing catalog or create a new one at the specified path.
    pub fn new(catalog_path: PathBuf) -> Result<Self, StorageError> {
        let catalog = if catalog_path.exists() {
            Catalog::load(&catalog_path).map_err(Self::convert_catalog_error)?
        } else {
            Catalog::new()
        };

        Ok(Self {
            catalog_path,
            catalog,
        })
    }

    /// Check if a relation exists in the catalog.
    pub fn relation_exists(&self, name: &str) -> bool {
        self.catalog.relation_exists(name)
    }

    /// Get metadata for a relation.
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

    /// Get detailed relation info (including heap file path).
    ///
    /// This is internal to the storage layer and not part of the public `StorageEngine` API.
    pub fn get_relation_info(
        &self,
        name: &str,
    ) -> Result<crate::storage::catalog::RelationMetadata, StorageError> {
        self.catalog
            .get_relation(name)
            .cloned()
            .map_err(Self::convert_catalog_error)
    }

    /// List all relations in the catalog.
    pub fn list_relations(&self) -> Vec<String> {
        self.catalog.list_relations()
    }

    /// Register a new relation in the catalog and save it.
    pub fn create_relation(
        &mut self,
        name: String,
        relation_type: RelationType,
        heap_file_path: PathBuf,
    ) -> Result<(), StorageError> {
        self.catalog
            .create_relation(name, relation_type, heap_file_path)
            .map_err(Self::convert_catalog_error)?;

        self.save()
    }

    /// Remove a relation from the catalog and save it.
    ///
    /// Returns the metadata of the dropped relation (useful for cleaning up files).
    pub fn drop_relation(
        &mut self,
        name: &str,
    ) -> Result<crate::storage::catalog::RelationMetadata, StorageError> {
        let metadata = self
            .catalog
            .drop_relation(name)
            .map_err(Self::convert_catalog_error)?;

        self.save()?;

        Ok(metadata)
    }

    /// Save the catalog to disk.
    fn save(&self) -> Result<(), StorageError> {
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
}
