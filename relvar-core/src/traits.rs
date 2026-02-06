//! Common traits for database components.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Trait for components that can execute queries.
///
/// This trait decouples components that need to query the database (like virtual relvars)
/// from the concrete `Database` struct, preventing circular dependencies.
pub trait QueryExecutor {
    /// Execute a query against a relation (base or virtual).
    ///
    /// # Errors
    ///
    /// Returns a [`DatabaseError`] if the query fails (e.g. relation not found).
    fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError>;
}
