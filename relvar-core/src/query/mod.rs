//! Query Builder and AST.
//!
//! This module provides a serializable AST ([`Query`]) and a fluent builder API
//! for constructing relational algebra queries. It allows queries to be
//! defined data-driven (e.g., from JSON), optimized, inspected, and executed.

pub mod ast;
pub mod error;

pub use ast::Query;
pub use error::QueryError;
