use crate::error::DatabaseError;
use crate::values::Relation;

/// Trait for executing queries against a database or other data source.
///
/// This trait abstracts the query execution capability, allowing components
/// like virtual relvars (views) and query ASTs to operate without depending
/// on the concrete `Database` struct or specific storage engines.
pub trait QueryExecutor {
    /// Execute a query for a named relation.
    ///
    /// This may return a base relation from storage or evaluate a virtual relvar.
    fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError>;
}
