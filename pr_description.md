💡 What: Replaced `Vec` parameters with `IntoIterator` in the query builder methods (`Query::project`, `Query::rename`, and `Query::summarize`).
🎯 Why: To eliminate unnecessary intermediate heap allocations. Previously, users had to allocate a `vec![]` just to build a query configuration.
📊 Impact: Callers can now pass stack-allocated arrays (e.g., `.project(["name", "email"])`) directly, reducing heap allocations during query construction.
🔬 Measurement: Run `cargo test` and `cargo test --doc` to verify that existing callers (using `vec![]`) still compile via backwards compatibility, and that the new array examples work flawlessly.
