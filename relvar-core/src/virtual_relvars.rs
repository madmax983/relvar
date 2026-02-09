//! Virtual relvar (view) definitions.

use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;

/// A function that evaluates a virtual relvar.
pub type VirtualRelvarEvaluator = fn(&mut dyn QueryExecutor) -> Result<Relation, DatabaseError>;

/// Definition of a virtual relvar (view).
///
/// TTM: RM Prescription 10 - Virtual relvars (views) re-evaluate their
/// defining expression on each query.
#[derive(Debug, Clone)]
pub struct VirtualRelvarDefinition {
    /// The name of the virtual relvar.
    pub name: String,
    /// The relation type (heading).
    pub relation_type: RelationType,
    /// The evaluation function that computes the virtual relvar's contents.
    ///
    /// Takes a mutable reference to a `QueryExecutor` (e.g., `Database`) and returns the computed relation.
    pub evaluator: VirtualRelvarEvaluator,
}
