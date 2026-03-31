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

pub(crate) mod ast;
pub(crate) mod builder;
pub(crate) mod execute;

pub use ast::{Query, QueryError};

#[cfg(test)]
mod tests;
