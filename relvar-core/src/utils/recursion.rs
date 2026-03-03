//! Recursion limits to prevent stack overflow attacks.
//!
//! Deeply nested types, values, or constraints can exhaust the stack and cause a Denial of Service.
//! These utilities ensure safe deserialization by imposing limits on nested structures.

use serde::Deserialize;
use std::cell::Cell;

thread_local! {
    static RECURSION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// The absolute maximum allowed depth for parsing deeply nested structures.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// A scope guard that tracks the current recursion depth.
///
/// It increments the depth upon creation and decrements it when dropped.
/// Ensures limits are respected during operations that parse recursive types.
///
/// # Examples
///
/// ```rust,ignore
/// use relvar_core::utils::recursion::RecursionGuard;
///
/// // Successfully get a guard (assuming we're not 64 layers deep)
/// let guard = RecursionGuard::new();
/// assert!(guard.is_ok());
/// ```
pub struct RecursionGuard;

impl RecursionGuard {
    /// Attempts to create a new guard. Fails if the recursion limit has been reached.
    ///
    /// # Returns
    ///
    /// Returns `Ok(RecursionGuard)` if the depth limit is not exceeded, or an error string
    /// indicating the limit was reached.
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

/// A wrapper type for `serde` deserialization that enforces a maximum recursion depth limit.
///
/// This provides protection against malicious inputs constructing
/// very deep structures to crash the program.
///
/// By wrapping any potentially recursive `struct` or `enum` field in `DepthGuarded<T>`,
/// serde will automatically run the recursion check before continuing down the parsing tree.
///
/// # Examples
///
/// ```rust,ignore
/// use relvar_core::utils::recursion::DepthGuarded;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// enum Expr {
///     Literal(i32),
///     // Adding `DepthGuarded` protects against deeply nested trees
///     Add(Box<DepthGuarded<Expr>>, Box<DepthGuarded<Expr>>),
/// }
/// ```
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
