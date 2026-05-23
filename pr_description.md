💡 What: The optimization implemented. Switched `Vec<String>` to `Vec<&str>` for identifying common attributes during join and semijoin operations. Modified `JoinKey` and `SemijoinKey` to use `&[&str]` internally instead of dynamically allocating strings.

🎯 Why: The bottleneck. Constructing a `Vec<String>` from attribute names requires cloning the strings representing the names, causing unnecessary intermediate heap allocations and string copying during hashing and matching phases of relational algebra operations, which is fundamentally against the zero-cost abstractions philosophy.

📊 Impact: Expected gain. Eliminates redundant intermediate heap allocations of string slices and their contents for all `join`, `theta_join`, `semijoin`, and `semidifference` operations by reusing memory and references correctly without fighting the borrow checker unnecessarily.

🔬 Measurement: How to verify. Run `cargo bench` and observe improvements in relational algebra benchmarks targeting joins, semi-joins, and semi-differences. Or `cargo test` to see tests pass cleanly.
