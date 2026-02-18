//! Virtual relvar (view) definition.

use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;

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
    /// Takes an object capable of executing queries (e.g., Database) and returns the computed relation.
    pub evaluator: fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError>,
}
