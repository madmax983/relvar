use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;
use std::fmt;
use std::sync::Arc;

/// Definition of a virtual relvar (view).
///
/// Stores the metadata required to evaluate a virtual relvar on demand.
#[derive(Clone)]
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
    /// `Arc<dyn Fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError> + Send + Sync>`
    ///
    /// - **Input**: A `&dyn QueryExecutor`, which allows the view
    ///   to query other relvars (base or virtual) in the database.
    /// - **Output**: A `Result` containing the computed `Relation`.
    ///
    /// # Safety
    ///
    /// The evaluator is passed a read-only reference (`&`), ensuring that
    /// viewing a relation cannot cause side effects (mutations) in the database.
    pub evaluator: Arc<dyn Fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError> + Send + Sync>,
}

impl fmt::Debug for VirtualRelvarDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualRelvarDefinition")
            .field("relation_type", &self.relation_type)
            .field("evaluator", &"<closure>")
            .finish()
    }
}
