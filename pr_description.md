💡 What: Updated `project`, `rename`, and `summarize` APIs in `relvar-core/src/query/mod.rs` to accept `IntoIterator` instead of `Vec`.
🎯 Why: Eliminates heap allocation overhead by allowing stack-allocated arrays to be passed directly without `vec![]` allocations.
📊 Impact: Reduces intermediate vector allocations significantly when building query plans.
🔬 Measurement: Verify tests run successfully using array literals.
