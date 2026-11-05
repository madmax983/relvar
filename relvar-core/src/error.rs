//! Error types for the relational model.

use crate::constraints::ConstraintManagerError;
use crate::storage_engine::StorageError;
use crate::values::arena::AllocError;
use crate::values::relation::RelationError;
use alloc::string::String;
use thiserror::Error;

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

    /// A [multiple assignment](crate::database::Database::multiple_assign)
    /// names the same relvar more than once.
    #[error("Relvar {0} is assigned more than once in one multiple assignment")]
    DuplicateAssignmentTarget(String),

    /// A database assertion was violated.
    ///
    /// The payload is a human-readable message naming the violated assertion
    /// and describing the business rule it enforces, e.g.
    /// `"Database assertion 'total_non_negative' violated: Sum of all account
    /// balances must be non-negative"`.
    #[error("{0}")]
    AssertionViolation(String),

    /// A database assertion could not be registered, evaluated or honoured
    /// for a reason other than a violation: a duplicate name, a reference
    /// to something that is not a base relvar, an ill-typed condition, or a
    /// drop of a relvar an assertion refers to. See
    /// [`AssertionError`](crate::constraints::AssertionError).
    #[error("{0}")]
    Assertion(crate::constraints::AssertionError),

    /// The database records a closure-backed database assertion that has
    /// not been registered since the database was opened.
    ///
    /// TTM: RM Prescriptions 16 and 23. A closure cannot be stored, only
    /// its name. Until the assertion is registered again with
    /// [`add_assertion`](crate::database::Database::add_assertion), or
    /// dropped with
    /// [`forget_assertion`](crate::database::Database::forget_assertion),
    /// the database cannot check its whole constraint, so every change is
    /// refused. See
    /// [`unregistered_assertions`](crate::database::Database::unregistered_assertions).
    #[error(
        "Database assertion '{0}' is recorded but not registered: register it with add_assertion or drop it with forget_assertion"
    )]
    UnregisteredAssertion(String),

    /// Arena allocation failure in the zero-alloc value layer (v0.8).
    ///
    /// The arena APIs surface [`AllocError`](crate::values::arena::AllocError)
    /// directly; this variant lets arena-backed operations compose with the
    /// crate-wide error type.
    #[error("Arena allocation failed: {0}")]
    AllocError(#[from] AllocError),

    /// Arena-native algebra operator failure (v0.8).
    ///
    /// The `*_in` operators in [`crate::algebra::arena_ops`] surface
    /// [`ArenaOpError`](crate::algebra::ArenaOpError); this variant lets them
    /// compose with the crate-wide error type.
    #[error("Arena algebra operator failed: {0}")]
    ArenaOpError(#[from] crate::algebra::ArenaOpError),
}
