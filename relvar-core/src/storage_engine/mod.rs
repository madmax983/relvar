//! Storage engine trait and implementations.
//!
//! This module defines the `StorageEngine` trait that abstracts over different
//! storage backends (in-memory, persistent, etc.). The trait provides the minimal
//! interface needed by the Database to store and retrieve relations.
//!
//! # TTM Compliance
//!
//! The storage engine is purely an implementation detail (RM Proscription 3).
//! The logical relational model is independent of how data is physically stored.

use crate::types::RelationType;
use crate::values::{Relation, Tuple};
use thiserror::Error;

pub(crate) mod in_memory;

pub use in_memory::InMemoryEngine;

/// Errors that can occur during storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// The relation already exists.
    #[error("Relation {0} already exists")]
    RelationAlreadyExists(String),

    /// The relation was not found.
    #[error("Relation {0} not found")]
    RelationNotFound(String),

    /// A relation error occurred.
    #[error("Relation error: {0}")]
    Relation(String),

    /// A general storage error occurred.
    #[error("Storage error: {0}")]
    Other(String),
}

/// Metadata about a stored relation.
#[derive(Debug, Clone)]
pub struct RelationMetadata {
    /// The name of the relation.
    pub name: String,
    /// The type (heading) of the relation.
    pub relation_type: RelationType,
}

/// Trait for storage engine backends.
///
/// This trait abstracts over different storage implementations:
/// - `InMemoryEngine` - Pure in-memory storage (no I/O)
/// - `PersistentEngine` - Disk-based storage (in relvar-storage crate)
///
/// # Example
///
/// ```
/// use relvar_core::storage_engine::{StorageEngine, InMemoryEngine};
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
///
/// let mut engine = InMemoryEngine::new();
///
/// let rel_type = RelationType::new(
///     TupleType::new().with_attribute("id", ScalarType::Int)
/// );
///
/// engine.create_relation("TEST", rel_type).unwrap();
/// assert!(engine.relation_exists("TEST"));
/// ```
pub trait StorageEngine: Send + Sync {
    /// The snapshot type used for transactions.
    type Snapshot: Send + Sync;

    /// Create a new relation.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationAlreadyExists` if a relation with this name
    /// already exists.
    fn create_relation(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), StorageError>;

    /// Drop an existing relation.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError>;

    /// Check if a relation exists.
    fn relation_exists(&self, name: &str) -> bool;

    /// Looks up the metadata associated with a specific relation.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError>;

    /// List all relation names.
    fn list_relations(&self) -> Vec<String>;

    /// Load a relation (scan all tuples).
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn load_relation(&self, name: &str) -> Result<Relation, StorageError>;

    /// Store a relation (replaces all tuples).
    ///
    /// This is used for bulk operations like delete/update that rebuild the relation.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError>;

    /// Insert a single tuple.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError>;

    /// Begin a transaction (save current state).
    ///
    /// # Errors
    ///
    /// Yields an error if transaction creation fails.
    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError>;

    /// Commit a transaction (discard snapshot).
    ///
    /// This is a no-op for most engines since we're not rolling back.
    ///
    /// # Errors
    ///
    /// Yields an error if commit fails.
    fn commit_transaction(&mut self, _snapshot: Self::Snapshot) -> Result<(), StorageError> {
        Ok(())
    }

    /// Rollback a transaction (restore snapshot).
    ///
    /// # Errors
    ///
    /// Yields an error if rollback fails.
    fn rollback_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError>;
}
