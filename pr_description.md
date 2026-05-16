💡 What
Modified `Query::project`, `Query::rename`, and `Query::summarize` in `relvar-core/src/query/mod.rs` to accept `IntoIterator` instead of `Vec`. Updated doc-tests and unit tests to pass stack-allocated arrays directly instead of using `vec!`.

🎯 Why
Using `IntoIterator` removes the requirement for callers to allocate an intermediate `Vec` on the heap (e.g., `vec!["name"]`), adhering closely to the principle of avoiding unnecessary allocations in the query builder hot paths.

📊 Impact
Removes redundant heap allocations and collection overhead whenever a user builds a query with `project`, `rename`, or `summarize` using arrays. Allows users to write zero-cost `query.project(["name"])` seamlessly.

🔬 Measurement
Review the changes to `query/mod.rs` confirming the API modification and the elimination of `vec!` macro calls in query builder doc examples and `tests/basic.rs`. Performance is fundamentally improved by eliminating user-side allocations.
