//! Virtual Relvar (View) Definitions.
//!
//! This module defines the metadata and evaluation structures for Virtual Relvars,
//! which are the relational equivalent of SQL Views. Unlike base relvars, virtual relvars
//! do not store data; they store an expression (a query) that is evaluated on demand.
use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;

/// Definition of a virtual relvar (view).
///
/// Stores the metadata required to evaluate a virtual relvar on demand.
#[derive(Debug, Clone)]
pub(crate) struct VirtualRelvarDefinition {
    /// The relation type (heading) of the view.
    ///
    /// This defines the schema of the result produced by the evaluator.
    /// The database uses this to validate queries against the view without
    /// needing to evaluate it first.
    pub relation_type: RelationType,

    /// The evaluation function (closure) that computes the view's contents.
    ///
    /// # Signature
    ///
    /// `fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError>`
    ///
    /// - **Input**: A `&dyn QueryExecutor`, which allows the view
    ///   to query other relvars (base or virtual) in the database.
    /// - **Output**: A `Result` containing the computed `Relation`.
    ///
    /// # Safety
    ///
    /// The evaluator is passed a read-only reference (`&`), ensuring that
    /// viewing a relation cannot cause side effects (mutations) in the database.
    pub evaluator: fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError>,
}
