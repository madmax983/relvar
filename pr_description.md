💡 What: Optimized `common_attributes` helper in `semijoin.rs` to return `Vec<&'a str>` instead of `Vec<String>`.

🎯 Why: To eliminate unnecessary heap allocations for string cloning when extracting common attribute names for read-only lookups in relational operations.

📊 Impact: Removes one string allocation per common attribute when performing semijoin or semidifference.

🔬 Measurement: Run `cargo test` to verify correctness.