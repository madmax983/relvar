//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **Pivot**: Operator to rotate unique values from one column into multiple columns.
//! - **Mock**: Tools for generating random relations for testing.
//! - **Spatial**: Spatial data types.

pub mod mock;
pub mod pivot;
pub mod spatial;
