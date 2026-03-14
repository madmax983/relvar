//! Virtual Relvar (View) Definitions.
//!
//! A virtual relvar (or "view" in SQL terminology) is a relation variable whose
//! value is not explicitly stored in the database. Instead, its value is defined
//! by a relational expression evaluated dynamically whenever the virtual relvar
//! is queried.
//!
//! # TTM Compliance
//!
//! In *The Third Manifesto*, virtual relvars are semantically indistinguishable
//! from base (stored) relvars when queried. This satisfies the Principle of
//! Interchangeability. Users querying a virtual relvar should not need to know
//! (nor care) that it is computed rather than stored.
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::types::RelationType;
use crate::values::Relation;

/// Definition of a virtual relvar (view).
///
/// Stores the metadata required to evaluate a virtual relvar on demand.
#[derive(Debug, Clone)]
pub(crate) struct VirtualRelvarDefinition<E: StorageEngine> {
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
    /// `fn(&Database<E>) -> Result<Relation, DatabaseError>`
    ///
    /// - **Input**: A `&Database<E>`, which allows the view
    ///   to query other relvars (base or virtual) in the database.
    /// - **Output**: A `Result` containing the computed `Relation`.
    ///
    /// # Safety
    ///
    /// The evaluator is passed a read-only reference (`&`), ensuring that
    /// viewing a relation cannot cause side effects (mutations) in the database.
    pub evaluator: fn(&Database<E>) -> Result<Relation, DatabaseError>,
}
