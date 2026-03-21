//! Bounded Recursion Safety.
//!
//! This module provides a simple `DepthGuarded` wrapper to enforce maximum recursion
//! limits (`MAX_TYPE_DEPTH`) on nested tree structures (like ASTs or nested Types).
//! This prevents stack overflows, particularly against deeply nested payload attacks.
use serde::Deserialize;
use std::cell::Cell;

thread_local! {
    static RECURSION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// The maximum allowed recursion depth for nested structures.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// A guard that automatically tracks and limits call depth to prevent stack overflows.
///
/// This struct uses a thread-local counter to track recursion depth. When created via [`RecursionGuard::new`],
/// it increments the counter. When dropped, it decrements the counter. If the depth exceeds
/// `MAX_RECURSION_DEPTH`, creation fails.
///
/// This is particularly useful for protecting the database engine against deeply nested or recursive
/// inputs (e.g. self-referencing expressions, deeply nested constraint types, or malicious payloads).
///
/// # Examples
///
/// ```
/// use relvar_core::utils::RecursionGuard;
///
/// // RecursionGuard is for internal use, but here is how it works conceptually:
/// let guard = RecursionGuard::new().expect("Should succeed at top level");
/// // The guard automatically decrements the depth when it is dropped.
/// ```
pub struct RecursionGuard;

impl RecursionGuard {
    /// Creates a new `RecursionGuard` to safely track call depth and prevent stack overflows.
    ///
    /// This method exists to defend the engine against deeply nested or recursive
    /// inputs (e.g. self-referencing expressions, deeply nested constraint types)
    /// that could trigger a stack overflow and crash the database process.
    ///
    /// # Errors
    /// Returns an error if the current thread's recursion depth exceeds `MAX_RECURSION_DEPTH`.
    ///
    /// # Examples
    /// ```text
    /// // RecursionGuard is for internal use, but here is how it works conceptually:
    /// // let guard = RecursionGuard::new().expect("Should succeed at top level");
    /// // The guard automatically decrements the depth when it is dropped.
    /// ```
    pub fn new() -> Result<Self, &'static str> {
        RECURSION_DEPTH.with(|cell| {
            let depth = cell.get();
            if depth >= MAX_RECURSION_DEPTH {
                Err("Recursion limit exceeded")
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

/// A wrapper that enforces recursion limits during Serde deserialization.
#[derive(Debug)]
pub struct DepthGuarded<T>(pub T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DepthGuarded<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _guard = RecursionGuard::new().map_err(serde::de::Error::custom)?;
        let value = T::deserialize(deserializer)?;
        Ok(DepthGuarded(value))
    }
}
