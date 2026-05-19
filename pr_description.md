# ⚡ Bolt: Semijoin Optimization

💡 What: Changed the `common_attributes` helper function in `semijoin.rs` to return a `Vec<&'a str>` instead of a `Vec<String>`. Updated the `SemijoinKey` struct to hold a slice of string references `&'a [&'a str]`.

🎯 Why: The previous implementation of `common_attributes` cloned the attribute names into owned Strings. By using references from the `Relation`'s heading directly, we eliminate an unnecessary `Vec<String>` heap allocation and the associated string cloning overhead per semijoin operation.

📊 Impact: Reduces heap allocations and cloning during semijoin, semidifference, and matching operations, making chaining and large queries faster.

🔬 Measurement: Verified with `cargo test` to ensure zero logical regressions and ran the benchmark `bench_semijoin` to confirm stability and performance.