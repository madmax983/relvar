//! Recursion depth protection for deserialization.
//!
//! This module provides [`RecursionGuard`] and [`DepthGuarded`] to prevent
//! stack overflow attacks during deserialization of deeply nested structures.

use serde::{Deserialize, Deserializer};
use std::cell::Cell;

// ------------------- Recursion Guard -------------------

thread_local! {
    static RECURSION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Maximum allowed recursion depth for deserialization.
pub const MAX_RECURSION_DEPTH: usize = 32;

/// A guard that increments recursion depth on creation and decrements on drop.
///
/// If `MAX_RECURSION_DEPTH` is exceeded, `new()` returns an error.
pub struct RecursionGuard;

impl RecursionGuard {
    /// Attempts to create a new recursion guard.
    ///
    /// # Errors
    ///
    /// Returns an error string if the recursion limit would be exceeded.
    pub fn new() -> Result<Self, String> {
        RECURSION_DEPTH.with(|cell| {
            let depth = cell.get();
            if depth >= MAX_RECURSION_DEPTH {
                Err("Recursion limit exceeded".to_string())
            } else {
                cell.set(depth + 1);
                Ok(RecursionGuard)
            }
        })
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        RECURSION_DEPTH.with(|cell| {
            let depth = cell.get();
            if depth > 0 {
                cell.set(depth - 1);
            }
        });
    }
}

/// A wrapper that enforces recursion depth limits during deserialization.
///
/// Wrap recursive fields in `Box<DepthGuarded<T>>` to ensure that
/// processing them doesn't blow the stack.
#[derive(Debug)]
pub struct DepthGuarded<T>(pub T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DepthGuarded<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _guard = RecursionGuard::new().map_err(serde::de::Error::custom)?;
        let value = T::deserialize(deserializer)?;
        Ok(DepthGuarded(value))
    }
}
