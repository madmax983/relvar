//! Core traits for Relvar.

use crate::error::DatabaseError;
use crate::values::Relation;

/// A trait for entities that can provide relations by name.
///
/// This abstracts the [`crate::database::Database`] to allow the [`crate::query::Query`]
/// AST to execute without depending on the full Database struct, breaking potential
/// circular dependencies.
pub trait RelationSource {
    /// Retrieve a relation by name.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError` if the relation cannot be found or loaded.
    fn query(&self, name: &str) -> Result<Relation, DatabaseError>;
}
