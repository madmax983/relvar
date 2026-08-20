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
//! use relvar_core::query::Query;
//! use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
//! use relvar_core::values::ScalarValue;
//! use relvar_core::types::ScalarType;
//! use relvar_core::database::Database;
//! use relvar_core::storage_engine::InMemoryEngine;
//! use relvar_core::tuple;
//!
//! # let mut db = Database::new(InMemoryEngine::new());
//! # use relvar_core::types::{RelationType, TupleType};
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
//! let result = query.execute(&db).unwrap();
//!
//! // Explain
//! println!("{}", query.explain());
//! ```

use crate::algebra::Aggregation;
use crate::constraints::{ConstraintExpression, ExpressionError};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::Relation;
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
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        input: Box<Query>,
        /// The predicate condition.
        predicate: ConstraintExpression,
    },

    /// Project specific attributes (Projection/SELECT).
    ///
    /// Corresponds to the π (pi) operator. Computes a relation with only
    /// the specified attributes.
    Project {
        /// The input query.
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
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
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
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
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        left: Box<Query>,
        /// The right relation.
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        right: Box<Query>,
    },

    /// Summarize (Group By + Aggregation).
    ///
    /// Groups tuples by the `group_by` attributes and computes aggregate
    /// values (Sum, Count, Avg, Min, Max) for each group.
    Summarize {
        /// The input query.
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
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

impl Query {
    /// Executes the query plan against the given relation source (e.g., Database).
    ///
    /// This method recursively evaluates the query tree.
    ///
    /// # Execution Process
    ///
    /// 1.  **Traverse**: The execution starts at the root node and recursively calls `execute` on children.
    /// 2.  **Scan**: Leaf nodes ([`Query::Scan`]) load base relations from the database.
    /// 3.  **Process**: Each internal node takes the resulting relation(s) from its children
    ///     and applies its relational algebra operator.
    /// 4.  **Optimize**: Some operators perform just-in-time optimization. For example,
    ///     [`Query::Restrict`] prepares its predicate for efficient evaluation.
    ///
    /// # Query Optimization
    ///
    /// The current implementation uses a simple "Volcano-style" iterator model (though materialized
    /// at each step). Optimization is currently limited to:
    ///
    /// - **Predicate Preparation**: `Restrict` pre-compiles `IN` lists into HashSets and
    ///   `LIKE` patterns into regex/matchers to avoid re-parsing per tuple.
    /// - **Push-down**: Not yet implemented. Future versions will push `Restrict` and `Project`
    ///   down the tree to minimize intermediate result sizes.
    ///
    /// # Errors
    ///
    /// Yields an error if:
    /// - A referenced relation does not exist.
    /// - A constraint expression is invalid (e.g., type mismatch).
    /// - An algebraic operation fails (e.g., joining incompatible types).
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::query::Query;
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// db.create_relvar("TEST", RelationType::new(heading)).unwrap();
    /// db.insert("TEST", tuple!{ x: 1i64 }).unwrap();
    ///
    /// let q = Query::scan("TEST");
    /// let rel = q.execute(&db).unwrap();
    /// assert_eq!(rel.cardinality(), 1);
    /// ```
    pub fn execute<E: StorageEngine>(&self, db: &Database<E>) -> Result<Relation, QueryError> {
        match self {
            Query::Scan(table_name) => Ok(db.query(table_name)?),
            Query::Restrict { input, predicate } => {
                let relation = input.execute(db)?;

                // Pre-compute optimized structures (HashSet for IN, Vec<char> for LIKE)
                // This prevents O(N*M) behavior for large IN lists or LIKE patterns
                let prepared = predicate.prepare();

                // Manually restrict the relation to handle and propagate evaluation errors
                let mut eval_error = None;
                let result = relation.restrict_into(|tuple| {
                    match prepared.evaluate(tuple) {
                        Ok(res) => res,
                        Err(e) => {
                            if eval_error.is_none() {
                                eval_error = Some(e);
                            }
                            false // Treat as false to continue, but we'll error out anyway
                        }
                    }
                });

                if let Some(err) = eval_error {
                    return Err(QueryError::Constraint(err));
                }

                Ok(result)
            }
            Query::Project { input, attributes } => {
                let relation = input.execute(db)?;
                let attrs_ref: Vec<&str> = attributes.iter().map(|s| s.as_str()).collect();
                Ok(relation.project_into(&attrs_ref))
            }
            Query::Rename { input, mappings } => {
                let relation = input.execute(db)?;
                let mappings_ref: Vec<(&str, &str)> = mappings
                    .iter()
                    .map(|(a, b)| (a.as_str(), b.as_str()))
                    .collect();
                Ok(relation.rename_into(&mappings_ref))
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
                // aggregations is already Vec<Aggregation>, so no conversion needed
                let group_by_ref: Vec<&str> = group_by.iter().map(|s| s.as_str()).collect();
                Ok(relation
                    .summarize(&group_by_ref, aggregations)
                    .map_err(|e| QueryError::Algebra(e.to_string()))?)
            }
        }
    }

    /// Yields a human-readable explanation of the query plan.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("TEST").project(vec!["x"]);
    /// let explanation = q.explain();
    /// assert!(explanation.contains("Project"));
    /// assert!(explanation.contains("Scan"));
    /// ```
    pub fn explain(&self) -> String {
        self.explain_recursive(0)
    }

    fn explain_recursive(&self, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        self.format_explanation_node(depth, &indent)
    }

    /// Helper to format the explanation node.
    fn format_explanation_node(&self, depth: usize, indent: &str) -> String {
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("USERS");
    /// ```
    pub fn scan(table: impl Into<String>) -> Self {
        Query::Scan(table.into())
    }

    /// Wraps the query in a Restrict operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    /// use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let condition = ConstraintExpression::Cmp {
    ///     left: "id".to_string(),
    ///     op: CmpOp::Eq,
    ///     right: ValueOrRef::Value(ScalarValue::Int(1))
    /// };
    /// let q = Query::scan("USERS").restrict(condition);
    /// ```
    pub fn restrict(self, predicate: ConstraintExpression) -> Self {
        Query::Restrict {
            input: Box::new(self),
            predicate,
        }
    }

    /// Wraps the query in a Project operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("USERS").project(vec!["name", "email"]);
    /// ```
    pub fn project<S: Into<String>>(self, attributes: Vec<S>) -> Self {
        Query::Project {
            input: Box::new(self),
            attributes: attributes.into_iter().map(|s| s.into()).collect(),
        }
    }

    /// Wraps the query in a Rename operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("USERS").rename(vec![("name", "full_name")]);
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let users = Query::scan("USERS");
    /// let orders = Query::scan("ORDERS");
    /// let joined = users.join(orders);
    /// ```
    pub fn join(self, right: Query) -> Self {
        Query::Join {
            left: Box::new(self),
            right: Box::new(right),
        }
    }

    /// Wraps the query in a Summarize operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let q = Query::scan("USERS").summarize(vec!["department"], vec![Aggregation::count("emp_count")]);
    /// ```
    pub fn summarize<S: Into<String>>(
        self,
        group_by: Vec<S>,
        aggregations: Vec<Aggregation>,
    ) -> Self {
        Query::Summarize {
            input: Box::new(self),
            group_by: group_by.into_iter().map(|s| s.into()).collect(),
            aggregations,
        }
    }
}
#[cfg(test)]
mod tests;
