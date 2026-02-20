//! Shared traits for Relvar.
//!
//! This module defines the core traits used to abstract database operations,
//! primarily to support decoupling and testing.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Trait for executing read-only queries.
///
/// # Purpose
///
/// This trait abstracts the ability to execute a query against a database or
/// other query execution context. Its primary purpose is to decouple
/// **Virtual Relvars (Views)** from the concrete `Database` implementation.
///
/// By receiving a `&dyn QueryExecutor` instead of `&Database`, view definitions:
/// 1.  Avoid circular type dependencies.
/// 2.  Are guaranteed to be read-only (they cannot mutate the database).
/// 3.  Can be tested with mock implementations.
///
/// # Example
///
/// ```
/// use relvar_core::values::Relation;
/// use relvar_core::error::DatabaseError;
/// use relvar_core::traits::QueryExecutor;
///
/// // A mock executor for testing views
/// struct MockExecutor;
///
/// impl QueryExecutor for MockExecutor {
///     fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
///         // Return a dummy relation or error
///         Err(DatabaseError::RelationNotFound(relation_name.to_string()))
///     }
/// }
///
/// // A view evaluator function accepts the trait object
/// fn my_view_evaluator(db: &dyn QueryExecutor) -> Result<Relation, DatabaseError> {
///     db.query("SOME_TABLE")
/// }
/// ```
pub trait QueryExecutor {
    /// Execute a query by relation name.
    ///
    /// Returns the relation corresponding to the given name.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation does not exist.
    fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError>;
}
