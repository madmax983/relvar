//! Shared trait definitions for inversion of control.
//!
//! This module contains central trait definitions used across the `relvar-core` codebase
//! to break circular dependencies and allow abstractions to interact generically.
//! The most critical trait is [`QueryExecutor`], which allows components like virtual relvars
//! to execute queries without depending directly on the concrete `Database` implementation.
use crate::error::DatabaseError;
use crate::values::Relation;

/// Trait for executing queries against a database or other data source.
///
/// This trait abstracts the query execution capability, allowing components
/// like virtual relvars (views) and query ASTs to operate without depending
/// on the concrete [`crate::database::Database`] struct or specific storage engines.
pub trait QueryExecutor {
    /// Execute a query for a named relation.
    ///
    /// This may return a base relation from storage or evaluate a virtual relvar.
    fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError>;
}
