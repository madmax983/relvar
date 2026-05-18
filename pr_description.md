# ⚡ Bolt: Remove intermediate Vec<String> allocation in group operations

💡 What: Changed the internal signature of `validate_group_request` to return a `Vec<&str>` instead of `Vec<String>`, allowing `grouping_attrs` to be passed as `&[&str]` through the call stack (`build_group_result_heading`, `compute_grouped_tuples`, `group_tuples`). This entirely removed the intermediate `Vec<String>` pre-allocation for `attr_names` and instead directly called `attr.to_string()` during attribute insertion.

🎯 Why: During relational grouping operations, extracting grouping attributes involved mapping and allocating a `Vec<String>`. `Tuple::new_unchecked` expects a `BTreeMap<String, ScalarValue>`, which natively consumes `String` keys. Pre-allocating all `attr_names` only to clone them later provided no benefit and simply created redundant intermediate heap allocations on the hot path for each tuple in a nested group.

📊 Impact: Reduces heap allocation overhead during group operations.

🔬 Measurement: Run benchmark `cargo bench --bench group`.