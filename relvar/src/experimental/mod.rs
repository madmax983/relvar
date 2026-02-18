//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **[`pivot`]**: Operator to rotate unique values from one column into multiple columns.
//!   Useful for creating cross-tabulation reports.
//! - **[`mock`]**: Tools for generating random relations for testing.
//!   Helps in performance testing and fuzzing.
//! - **[`spatial`]**: Experimental spatial data types (`Point`) implemented using
//!   Relation-Valued Attributes (RVAs) and User-Defined Types (UDTs).

pub mod mock;
pub mod pivot;
pub mod spatial;
