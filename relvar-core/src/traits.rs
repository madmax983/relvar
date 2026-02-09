//! Traits for the relational model.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Trait for executing queries.
///
/// This trait abstracts the ability to query relvars, decoupling virtual relvars (views)
/// from the concrete `Database` implementation.
pub trait QueryExecutor {
    /// Query a relation (base or virtual).
    fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError>;

    /// Check if a relvar exists.
    fn relvar_exists(&self, name: &str) -> bool;
}
