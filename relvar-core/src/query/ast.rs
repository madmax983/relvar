use crate::algebra::Aggregation;
use crate::constraints::{ConstraintExpression, ExpressionError};
use crate::error::DatabaseError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AST for a relational query.
///
/// This structure represents a query plan that can be serialized, inspected,
/// optimized, and executed against a database. It forms a tree where leaf nodes
/// are relation scans and internal nodes are algebraic operations.
///
/// # Examples
///
/// ```
///
/// use relvar_core::query::Query;
///
/// let q = Query::scan("users");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    /// Scan a relation variable (base table).
    ///
    /// This is typically the leaf node of a query tree. It loads a relation
    /// by name from the database.
    Scan(String),

    /// Restrict tuples based on a predicate (Selection/WHERE).
    ///
    /// Corresponds to the σ (sigma) operator. Filters tuples from the input
    /// relation where the `predicate` evaluates to true.
    Restrict {
        /// The input query to filter.
        input: Box<Query>,
        /// The predicate condition.
        predicate: ConstraintExpression,
    },

    /// Project specific attributes (Projection/SELECT).
    ///
    /// Corresponds to the π (pi) operator. Returns a relation with only
    /// the specified attributes.
    Project {
        /// The input query.
        input: Box<Query>,
        /// The list of attribute names to keep.
        attributes: Vec<String>,
    },

    /// Rename attributes.
    ///
    /// Corresponds to the ρ (rho) operator. Renames attributes in the relation
    /// heading. Useful for preventing name collisions before a join or
    /// self-join.
    Rename {
        /// The input query.
        input: Box<Query>,
        /// Mapping of (old_name, new_name).
        mappings: Vec<(String, String)>,
    },

    /// Natural Join two relations.
    ///
    /// Corresponds to the ⨝ operator. Combines tuples from two relations
    /// based on common attribute names. If there are no common attributes,
    /// this becomes a Cartesian product.
    Join {
        /// The left relation.
        left: Box<Query>,
        /// The right relation.
        right: Box<Query>,
    },

    /// Summarize (Group By + Aggregation).
    ///
    /// Groups tuples by the `group_by` attributes and computes aggregate
    /// values (Sum, Count, Avg, Min, Max) for each group.
    Summarize {
        /// The input query.
        input: Box<Query>,
        /// Attributes to group by.
        group_by: Vec<String>,
        /// Aggregations to perform.
        aggregations: Vec<Aggregation>,
    },
}

/// Errors that can occur during query execution.
#[derive(Debug, Error)]
pub enum QueryError {
    /// Error from the database engine.
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    /// Error evaluating a constraint expression.
    #[error("Constraint evaluation error: {0}")]
    Constraint(#[from] ExpressionError),

    /// Error in relational algebra operation.
    #[error("Algebra error: {0}")]
    Algebra(String),
}
