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
use alloc::string::String;
use alloc::vec::Vec;

pub(crate) mod in_memory;

pub use in_memory::InMemoryEngine;

/// Isolation level of a transaction.
///
/// TTM treats each transaction as an atomic, isolated unit of work. The
/// levels below refine what "isolated" means when transactions overlap in
/// time; stronger levels hide more concurrent activity at the cost of more
/// aborted transactions.
///
/// Guarantees (from weakest to strongest):
///
/// - [`IsolationLevel::ReadCommitted`]: every read observes the latest
///   committed state. A transaction can observe a tuple inserted (or
///   removed) by a transaction that commits mid-way through it —
///   non-repeatable reads and phantoms are possible.
/// - [`IsolationLevel::RepeatableRead`]: the transaction observes a fixed
///   snapshot taken when it begins. Re-reading any data returns the same
///   result for the life of the transaction, no matter what other
///   transactions commit meanwhile.
/// - [`IsolationLevel::Serializable`]: everything repeatable read
///   guarantees, plus the commit is rejected with
///   [`StorageError::SerializationFailure`] when a concurrent transaction
///   committed changes that overlap this transaction's read or write sets.
///   The effect is as if overlapping transactions had run one after the
///   other (first-committer-wins).
///
/// The default is [`IsolationLevel::RepeatableRead`], matching the
/// historical behavior of [`Database::begin`].
///
/// See [`StorageEngine::begin_transaction_with_isolation`] and
/// [`Database::begin_transaction_with_level`](crate::database::Database::begin_transaction_with_level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IsolationLevel {
    /// Each read observes the latest committed state.
    ReadCommitted,
    /// All reads observe the transaction's begin snapshot.
    #[default]
    RepeatableRead,
    /// Repeatable read plus first-committer-wins conflict validation.
    Serializable,
}

/// The grim reality of physical storage.
///
/// Even the most pristine relational algebra must eventually touch the cold, hard disk.
/// This enum represents the various ways persistence can fail. From the tragic
/// loss of a file (`IoError`) to the phantom menace of a missing relation
/// (`RelationNotFound`).
///
/// # Recovery
/// - **RelationNotFound:** Double check your system catalog. Are you querying a relvar that hasn't been defined yet? Ensure `create_relvar` was called.
/// - **RelationAlreadyExists:** You're attempting to define a relvar with a name that is already taken. Choose a unique name or drop the existing one.
///
/// # Examples
///
/// ```
/// use relvar_core::storage_engine::StorageError;
///
/// // An attempt to query a relation that the Storage Engine cannot find in its catalog.
/// // The proper response is to ensure the table was created before querying.
/// let err = StorageError::RelationNotFound("EMPLOYEES".to_string());
/// assert!(err.to_string().contains("EMPLOYEES not found"));
/// ```
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

    /// A transaction failed serializable validation: a concurrent
    /// transaction committed changes overlapping this transaction's read
    /// or write sets. Retry the transaction.
    #[error("Serialization failure: {0}")]
    SerializationFailure(String),

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
/// # Examples
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

    /// Get metadata for a relation.
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
    /// Equivalent to
    /// [`begin_transaction_with_isolation`](Self::begin_transaction_with_isolation)
    /// at [`IsolationLevel::default`].
    ///
    /// # Errors
    ///
    /// Yields an error if transaction creation fails.
    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError>;

    /// Begin a transaction at the given isolation level
    /// (TTM transaction semantics).
    ///
    /// The isolation level controls which concurrent changes this
    /// transaction can observe while it runs:
    ///
    /// - [`IsolationLevel::ReadCommitted`]: each read observes the latest
    ///   committed state.
    /// - [`IsolationLevel::RepeatableRead`]: all reads observe the state as
    ///   of the transaction's start.
    /// - [`IsolationLevel::Serializable`]: like repeatable read, plus the
    ///   commit fails with
    ///   [`StorageError::SerializationFailure`] when a concurrent
    ///   transaction committed changes that overlap this transaction's
    ///   read/write sets (first-committer-wins).
    ///
    /// The default implementation ignores the level and delegates to
    /// [`begin_transaction`](Self::begin_transaction), preserving the
    /// engine's existing snapshot semantics. Engines with genuine
    /// multi-version concurrency control should override this method.
    ///
    /// # Errors
    ///
    /// Yields an error if transaction creation fails, or if the engine
    /// cannot provide the requested level.
    fn begin_transaction_with_isolation(
        &mut self,
        _level: IsolationLevel,
    ) -> Result<Self::Snapshot, StorageError> {
        self.begin_transaction()
    }

    /// Commit a transaction (discard snapshot).
    ///
    /// # Contract
    ///
    /// If this method returns `Err`, the transaction is no longer active in
    /// the engine: the engine must have ended it (aborted, best-effort)
    /// before returning the error, and the snapshot is consumed. Callers
    /// such as [`Database::commit`](crate::database::Database::commit) rely
    /// on this to drop their own transaction state without stranding an
    /// engine-side transaction.
    ///
    /// # Errors
    ///
    /// Yields an error if commit fails — e.g.
    /// [`StorageError::SerializationFailure`] when a serializable
    /// transaction's commit validation rejects it.
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
