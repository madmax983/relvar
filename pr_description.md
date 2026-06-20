⚡ Bolt: rename_into
💡 What: Switched `Relation::rename` to `Relation::rename_into` in `Query::execute`.
🎯 Why: In `Query::execute`, the intermediate relation is owned. Using `rename` cloned strings and tuples, while `rename_into` consumes the relation in-place and reuses the old strings if names didn't change.
📊 Impact: Reduces heap allocations and `.clone()` overhead during query renaming operations.
🔬 Measurement: Run `cargo test -p relvar-core`.
