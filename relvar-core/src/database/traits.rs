use crate::values::Relation;
use crate::types::RelationType;
use crate::database::DatabaseError;

/// Trait for querying relations.
///
/// This trait abstracts the ability to query a relation, allowing components
/// (like virtual relvars) to access data without depending on the full `Database` struct.
pub trait QueryContext {
    /// Query a relation by name.
    ///
    /// Returns the relation if found, or an error.
    fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError>;

    /// Get the type (heading) of a relation.
    fn relation_type(&self, relation_name: &str) -> Result<RelationType, DatabaseError>;
}
