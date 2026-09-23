//! Error types for the relational model.

use crate::constraints::ConstraintManagerError;
use crate::storage_engine::StorageError;
use crate::values::relation::RelationError;
use thiserror::Error;
use alloc::string::String;

/// Errors that can occur during database operations.
///
/// # Examples
///
/// ```
/// use relvar_core::error::DatabaseError;
///
/// let err = DatabaseError::RelationAlreadyExists("USERS".to_string());
/// assert_eq!(err.to_string(), "Relation USERS already exists");
///
/// match err {
///     DatabaseError::RelationAlreadyExists(name) => {
///         assert_eq!(name, "USERS");
///     }
///     _ => unreachable!(),
/// }
/// ```
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

    /// Constraint violation.
    #[error("Constraint violation: {0}")]
    Constraint(#[from] ConstraintManagerError),

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

    /// Error during algebraic operation
    #[error("Algebra error: {0}")]
    AlgebraError(String),

    /// A database assertion was violated.
    ///
    /// The payload is a human-readable message naming the violated assertion
    /// and describing the business rule it enforces, e.g.
    /// `"Database assertion 'total_non_negative' violated: Sum of all account
    /// balances must be non-negative"`.
    #[error("{0}")]
    AssertionViolation(String),
}
