//! Shared traits for Relvar.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Trait for executing queries.
///
/// This trait abstracts the ability to execute a query against a database or
/// other query execution context. It allows decoupling components like
/// virtual relvars from the concrete `Database` implementation.
pub trait QueryExecutor {
    /// Execute a query by relation name.
    ///
    /// returns the relation corresponding to the given name.
    fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError>;
}
