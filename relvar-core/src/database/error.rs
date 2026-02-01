//! Database errors.

use crate::constraints::CheckConstraintError;
use crate::storage_engine::StorageError;
use crate::values::relation::RelationError;
use thiserror::Error;

/// Errors that can occur during database operations.
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// A storage error occurred.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// An error occurred with a relation value.
    #[error("Relation error: {0}")]
    Relation(#[from] RelationError),

    /// Attempted to create a relation that already exists.
    #[error("Relation {0} already exists")]
    RelationAlreadyExists(String),

    /// The specified relation does not exist.
    #[error("Relation {0} not found")]
    RelationNotFound(String),

    /// The tuple's type does not match the relation's heading.
    #[error("Tuple type does not match relation type")]
    TupleMismatch,

    /// Primary key constraint violation.
    #[error("Primary key violation")]
    PrimaryKeyViolation,

    /// Candidate key constraint violation.
    #[error("Candidate key violation")]
    CandidateKeyViolation,

    /// Foreign key constraint violation.
    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    /// Type constraint violation.
    #[error("Type constraint violation: {0}")]
    TypeConstraintViolation(String),

    /// CHECK constraint violation.
    #[error("CHECK constraint violation: {0}")]
    CheckConstraintViolation(#[from] CheckConstraintError),

    /// Transaction error.
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Cannot modify a virtual relvar.
    #[error("Cannot modify virtual relvar {0}")]
    CannotModifyVirtualRelvar(String),

    /// Cannot drop a system relvar.
    #[error("Cannot drop system relvar {0}")]
    CannotDropSystemRelvar(String),

    /// Duplicate attribute name.
    #[error("Duplicate attribute name: {0}")]
    DuplicateAttributeName(String),

    /// The attribute does not exist.
    #[error("Attribute {0} not found in relation {1}")]
    AttributeNotFound(String, String),
}
