//! Query Builder and AST for relational queries.
//!
//! This module is deprecated. Use `relvar::query` instead.

#![allow(deprecated)]

#[deprecated(since = "0.2.0", note = "Use `relvar::query` instead")]
pub use relvar_core::query::{Query, QueryError};

// Re-exports for backward compatibility
use crate::algebra::summarize::{Aggregation, AggregationFn};

/// Deprecated: Use `relvar::algebra::summarize::Aggregation` instead.
#[deprecated(since = "0.2.0", note = "Use `relvar::algebra::summarize::Aggregation` instead")]
pub type QueryAggregation = Aggregation;

/// Deprecated: Use `relvar::algebra::summarize::AggregationFn` instead.
#[deprecated(since = "0.2.0", note = "Use `relvar::algebra::summarize::AggregationFn` instead")]
pub type QueryAggregationFn = AggregationFn;
