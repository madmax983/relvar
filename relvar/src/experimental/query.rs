//! Query Builder and AST for relational queries.
//!
//! This module provides a serializable AST ([`Query`]) and a fluent builder API
//! for constructing relational algebra queries. It allows queries to be
//! defined data-driven (e.g., from JSON), optimized, inspected ([`Query::explain`]),
//! and executed against a [`Database`].
//!
//! # Example
//!
//! ```
//! use relvar::experimental::query::{Query, QueryAggregation, QueryAggregationFn};
//! use relvar::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
//! use relvar::values::ScalarValue;
//! use relvar::types::ScalarType;
//! use relvar::{Database, InMemoryEngine, tuple};
//!
//! # let mut db = Database::new(InMemoryEngine::new());
//! # use relvar::types::{RelationType, TupleType};
//! # let heading = TupleType::new()
//! #    .with_attribute("id", ScalarType::Int)
//! #    .with_attribute("name", ScalarType::String);
//! # db.create_relvar("USERS", RelationType::new(heading)).unwrap();
//!
//! // Build a query: SELECT name FROM USERS WHERE id = 1
//! let query = Query::scan("USERS")
//!     .restrict(ConstraintExpression::Cmp {
//!         left: "id".to_string(),
//!         op: CmpOp::Eq,
//!         right: ValueOrRef::Value(ScalarValue::Int(1)),
//!     })
//!     .project(vec!["name"]);
//!
//! // Execute
//! let result = query.execute(&mut db).unwrap();
//!
//! // Explain
//! println!("{}", query.explain());
//! ```

use crate::algebra::summarize::{Aggregation, AggregationFn};
use crate::constraints::{ConstraintExpression, ExpressionError};
use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::{Database, Relation, StorageEngine};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AST for a relational query.
///
/// This structure represents a query plan that can be serialized, inspected,
/// optimized, and executed against a database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    /// Scan a relation variable (base table).
    Scan(String),

    /// Restrict tuples based on a predicate (Selection/WHERE).
    Restrict {
        /// The input query to filter.
        input: Box<Query>,
        /// The predicate condition.
        predicate: ConstraintExpression,
    },

    /// Project specific attributes (Projection/SELECT).
    Project {
        /// The input query.
        input: Box<Query>,
        /// The list of attribute names to keep.
        attributes: Vec<String>,
    },

    /// Rename attributes.
    Rename {
        /// The input query.
        input: Box<Query>,
        /// Mapping of (old_name, new_name).
        mappings: Vec<(String, String)>,
    },

    /// Natural Join two relations.
    Join {
        /// The left relation.
        left: Box<Query>,
        /// The right relation.
        right: Box<Query>,
    },

    /// Summarize (Group By + Aggregation).
    Summarize {
        /// The input query.
        input: Box<Query>,
        /// Attributes to group by.
        group_by: Vec<String>,
        /// Aggregations to perform.
        aggregations: Vec<QueryAggregation>,
    },
}

/// Serializable representation of an aggregation function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryAggregationFn {
    /// Count the number of tuples.
    Count,
    /// Sum an integer attribute.
    Sum(String),
    /// Average an integer attribute.
    Avg(String),
    /// Minimum value of an attribute.
    Min(String),
    /// Maximum value of an attribute.
    Max(String),
}

/// Serializable representation of an aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAggregation {
    /// The name of the resulting attribute.
    pub result_name: String,
    /// The type of the resulting attribute.
    pub result_type: ScalarType,
    /// The aggregation function to apply.
    pub function: QueryAggregationFn,
}

impl From<QueryAggregation> for Aggregation {
    fn from(q: QueryAggregation) -> Self {
        let function = match q.function {
            QueryAggregationFn::Count => AggregationFn::Count,
            QueryAggregationFn::Sum(attr) => AggregationFn::Sum(attr),
            QueryAggregationFn::Avg(attr) => AggregationFn::Avg(attr),
            QueryAggregationFn::Min(attr) => AggregationFn::Min(attr),
            QueryAggregationFn::Max(attr) => AggregationFn::Max(attr),
        };
        Aggregation {
            result_name: q.result_name,
            result_type: q.result_type,
            function,
        }
    }
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

impl Query {
    /// Executes the query plan against the given database.
    pub fn execute<S: StorageEngine>(&self, db: &mut Database<S>) -> Result<Relation, QueryError> {
        match self {
            Query::Scan(table_name) => Ok(db.query(table_name)?),
            Query::Restrict { input, predicate } => {
                let relation = input.execute(db)?;
                // Note: Relation::restrict takes a closure that returns bool.
                // We use evaluate() inside, but we must handle errors.
                // Currently, we treat evaluation errors as false (exclude tuple).
                let predicate = predicate.clone();
                let result = relation
                    .restrict(move |tuple| predicate.evaluate(tuple).unwrap_or_default());
                Ok(result)
            }
            Query::Project { input, attributes } => {
                let relation = input.execute(db)?;
                let attrs_ref: Vec<&str> = attributes.iter().map(|s| s.as_str()).collect();
                Ok(relation.project(&attrs_ref))
            }
            Query::Rename { input, mappings } => {
                let relation = input.execute(db)?;
                let mappings_ref: Vec<(&str, &str)> = mappings
                    .iter()
                    .map(|(a, b)| (a.as_str(), b.as_str()))
                    .collect();
                Ok(relation.rename(&mappings_ref))
            }
            Query::Join { left, right } => {
                let left_rel = left.execute(db)?;
                let right_rel = right.execute(db)?;
                Ok(left_rel.join(&right_rel)?)
            }
            Query::Summarize {
                input,
                group_by,
                aggregations,
            } => {
                let relation = input.execute(db)?;
                let aggs: Vec<Aggregation> = aggregations.iter().cloned().map(Into::into).collect();
                let group_by_ref: Vec<&str> = group_by.iter().map(|s| s.as_str()).collect();
                Ok(relation
                    .summarize(&group_by_ref, &aggs)
                    .map_err(|e| QueryError::Algebra(e.to_string()))?)
            }
        }
    }

    /// Returns a human-readable explanation of the query plan.
    pub fn explain(&self) -> String {
        self.explain_recursive(0)
    }

    fn explain_recursive(&self, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        match self {
            Query::Scan(table) => format!("{}Scan({})", indent, table),
            Query::Restrict { input, predicate } => {
                format!(
                    "{}Restrict({:?})\n{}",
                    indent,
                    predicate,
                    input.explain_recursive(depth + 1)
                )
            }
            Query::Project { input, attributes } => {
                format!(
                    "{}Project({:?})\n{}",
                    indent,
                    attributes,
                    input.explain_recursive(depth + 1)
                )
            }
            Query::Rename { input, mappings } => {
                format!(
                    "{}Rename({:?})\n{}",
                    indent,
                    mappings,
                    input.explain_recursive(depth + 1)
                )
            }
            Query::Join { left, right } => {
                format!(
                    "{}Join\n{}\n{}",
                    indent,
                    left.explain_recursive(depth + 1),
                    right.explain_recursive(depth + 1)
                )
            }
            Query::Summarize {
                input,
                group_by,
                aggregations,
            } => {
                format!(
                    "{}Summarize(group_by={:?}, aggs={:?})\n{}",
                    indent,
                    group_by,
                    aggregations,
                    input.explain_recursive(depth + 1)
                )
            }
        }
    }

    /// Creates a Scan query.
    pub fn scan(table: impl Into<String>) -> Self {
        Query::Scan(table.into())
    }

    /// Wraps the query in a Restrict operation.
    pub fn restrict(self, predicate: ConstraintExpression) -> Self {
        Query::Restrict {
            input: Box::new(self),
            predicate,
        }
    }

    /// Wraps the query in a Project operation.
    pub fn project<S: Into<String>>(self, attributes: Vec<S>) -> Self {
        Query::Project {
            input: Box::new(self),
            attributes: attributes.into_iter().map(|s| s.into()).collect(),
        }
    }

    /// Wraps the query in a Rename operation.
    pub fn rename<S1: Into<String>, S2: Into<String>>(self, mappings: Vec<(S1, S2)>) -> Self {
        Query::Rename {
            input: Box::new(self),
            mappings: mappings
                .into_iter()
                .map(|(a, b)| (a.into(), b.into()))
                .collect(),
        }
    }

    /// Joins this query with another query.
    pub fn join(self, right: Query) -> Self {
        Query::Join {
            left: Box::new(self),
            right: Box::new(right),
        }
    }

    /// Wraps the query in a Summarize operation.
    pub fn summarize<S: Into<String>>(
        self,
        group_by: Vec<S>,
        aggregations: Vec<QueryAggregation>,
    ) -> Self {
        Query::Summarize {
            input: Box::new(self),
            group_by: group_by.into_iter().map(|s| s.into()).collect(),
            aggregations,
        }
    }
}
