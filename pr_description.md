💡 What: The optimization implemented
Replaced `Vec<T>` arguments in `Query::project`, `Query::rename`, and `Query::summarize` with generic `IntoIterator<Item = T>` traits.

🎯 Why: The bottleneck
Forcing a `Vec` for small, statically known argument lists (like projection attributes) requires a heap allocation at the call site (e.g., `vec!["name", "email"]`).

📊 Impact: Expected gain
Eliminates intermediate heap allocations when users pass stack-allocated arrays like `["name", "email"]`. Zero runtime cost for existing `vec!` callers.

🔬 Measurement: How to verify
Run `cargo bench` on query builder usage, and check `cargo run` examples to see that array syntax is supported.
