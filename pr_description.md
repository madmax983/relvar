💡 What
Modified `join` and `semijoin` operations (including `theta_join`, `semijoin_into`, and `semidifference_into`) to use `Vec<&str>` instead of allocating `Vec<String>` when extracting common attributes. The `JoinKey` and `SemijoinKey` structs were also updated to hold `&[&str]` slices.

🎯 Why
When executing relational algebra operations like joins, identifying common attributes between two relations previously mapped the attribute names to `String` using `.cloned()`. This incurred unnecessary heap allocations proportional to the number of common attributes in the relations on every single join operation, acting as a performance bottleneck.

📊 Impact
Eliminates `String` heap allocations during the setup phase of `join`, `theta_join`, `semijoin`, `semidifference` operations by zero-cost abstracting over string slices from the underlying attribute schema. Memory allocations and time taken per operation will be measurably reduced.

🔬 Measurement
Review `cargo clippy`, `cargo test`, and `cargo bench` to observe the removed allocations and ensure correctness remains intact with the new slice mappings.
