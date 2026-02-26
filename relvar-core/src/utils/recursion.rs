use std::cell::Cell;

thread_local! {
    static RECURSION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Maximum recursion depth allowed before aborting to prevent stack overflow.
/// This limit is set conservatively to 32 to support common use cases while
/// ensuring safety on systems with limited stack space.
pub const MAX_RECURSION_DEPTH: usize = 32;

/// A RAII guard that tracks recursion depth on the current thread.
///
/// When a `RecursionGuard` is created, it increments the thread-local recursion counter.
/// When it is dropped, it decrements the counter. If the depth exceeds
/// [`MAX_RECURSION_DEPTH`], creation fails.
///
/// This is used to protect against stack overflow attacks in recursive structures like
/// [`ScalarValue`](crate::values::ScalarValue) and [`ConstraintExpression`](crate::constraints::expression::ConstraintExpression).
pub struct RecursionGuard;

impl RecursionGuard {
    /// Attempts to create a new recursion guard.
    ///
    /// # Returns
    ///
    /// * `Ok(RecursionGuard)` - If the current recursion depth is within limits.
    /// * `Err(&'static str)` - If the recursion limit has been exceeded.
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
