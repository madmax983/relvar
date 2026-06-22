⚡ Bolt: Use rename_into for Query Rename execution

💡 **What**: Changed `Query::execute` for `Rename` to use `relation.rename_into` instead of `relation.rename`.
🎯 **Why**: The `rename` method takes `&self` and allocates a new `Relation` with cloned `String`s and tuples. Since the intermediate relation is owned and dropped immediately after, we can use `rename_into` which consumes the `Relation` by value, reusing internal allocations where names haven't changed.
📊 **Impact**: Reduces heap allocations when evaluating `Query::Rename` operators, particularly for relations with many tuples. Benchmarks show a significant throughput increase and execution time reduction.
🔬 **Measurement**: Verified via `cargo bench --bench algebra -- rename`.
