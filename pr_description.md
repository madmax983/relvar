⚡ Bolt: [Optimization] Use rename_into for Query::Rename execution

💡 What: Switched `relation.rename()` to `relation.rename_into()` in `Query::execute` for `Query::Rename`.
🎯 Why: `Query::execute` owns the intermediate relation. Calling `.rename()` borrows it and forces the allocation of new strings and values for every tuple. `rename_into()` consumes the relation, reusing internal `HashSet` memory and string allocations for unmodified attributes.
📊 Impact: Eliminates a full intermediate heap allocation (and tuple value clones) per Rename operation in the query engine.
🔬 Measurement: Run `cargo bench` or `cargo test` to verify correctness and see fewer allocations.
