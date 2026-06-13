//! General utility functions and common structures.
//!
//! This module contains helper components that do not inherently belong to relational
//! logic but are used across the codebase, such as memory bounds checking or recursion limits.
pub mod recursion;

pub use recursion::RecursionGuard;
