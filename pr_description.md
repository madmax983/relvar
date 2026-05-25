💡 What
Updated the `project`, `rename`, and `summarize` methods in `relvar-core::query::Query` to accept `IntoIterator` arguments instead of requiring concrete `Vec` allocations. Also fixed `summarize` tests to avoid intermediate `.collect::<Vec<_>>()` array slicing allocations.

🎯 Why
By accepting `IntoIterator`, callers can now pass stack-allocated arrays (e.g., `["a", "b"]`) or slices directly to the query builder API without needing to wrap them in `vec![]` calls. This eliminates redundant heap allocations across the codebase when building queries.

📊 Impact
- Eliminates 1 heap allocation per `project` call when passing array literals.
- Eliminates 1 heap allocation per `rename` call when passing array literals.
- Eliminates 2 heap allocations per `summarize` call when passing array literals for both `group_by` and `aggregations`.
- Test optimizations eliminate intermediate `Vec` collections by leveraging iterator slicing directly.

🔬 Measurement
Review the changes in `relvar-core/src/query/mod.rs` and the optimized tests in `relvar-core/src/algebra/summarize.rs`. Ensure `cargo check` and `cargo test` pass successfully.