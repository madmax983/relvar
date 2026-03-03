//! Virtual Relvars (Views) implementation.
//!
//! A virtual relvar is a relation variable whose value is defined by a
//! relational expression rather than being stored physically. Every time
//! a virtual relvar is evaluated, it computes its value based on the
//! current state of the database.
//!
//! Virtual relvars provide logical data independence by shielding
//! users from changes in the underlying base relvars.

use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;

/// Definition of a virtual relvar (view).
///
/// Stores the metadata required to evaluate a virtual relvar on demand.
/// This struct is kept internal to the database layer. Users create views
/// using [`crate::database::Database::define_virtual_relvar`].
///
/// # Architecture
///
/// A virtual relvar doesn't store tuples on disk. Instead, it stores an `evaluator`
/// function (a closure) that computes the relation dynamically when queried.
/// It also stores a [`RelationType`] to define its schema upfront, allowing
/// for type checking without needing to execute the evaluator.
///
/// # Examples
///
/// Internally, a definition looks like this:
///
/// ```rust,ignore
/// use relvar_core::types::{RelationType, TupleType};
/// use relvar_core::database::virtual_relvar::VirtualRelvarDefinition;
///
/// let my_view = VirtualRelvarDefinition {
///     relation_type: RelationType::new(TupleType::new()),
///     evaluator: |executor| {
///         // Execute logic to build the view relation dynamically
///         executor.query("BASE_TABLE")
///     }
/// };
/// ```
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
    /// - **Input**: A reference implementing [`QueryExecutor`], which allows the view
    ///   to query other relvars (base or virtual) in the database.
    /// - **Output**: A [`Result`] containing the computed [`Relation`].
    ///
    /// # Safety
    ///
    /// The evaluator is passed a read-only reference (`&`), ensuring that
    /// viewing a relation cannot cause side effects (mutations) in the database.
    pub evaluator: fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError>,
}
