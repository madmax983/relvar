//! Virtual relvar (view) definition.

use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::types::RelationType;
use crate::values::Relation;

/// Definition of a virtual relvar (view).
///
/// TTM: RM Prescription 10 - Virtual relvars (views) re-evaluate their
/// defining expression on each query.
#[derive(Debug, Clone)]
pub struct VirtualRelvarDefinition<E: StorageEngine> {
    /// The name of the virtual relvar.
    pub name: String,
    /// The relation type (heading).
    pub relation_type: RelationType,
    /// The evaluation function that computes the virtual relvar's contents.
    ///
    /// Takes a database reference and returns the computed relation.
    pub evaluator: fn(&Database<E>) -> Result<Relation, DatabaseError>,
}
