⚡ Bolt: [Remove clone during DML update operation]

💡 What:
Introduced an `into_parts()` method to `Relation` that consumes it and returns its components (`RelationType` and `HashSet<Tuple>`). Used this new method in `compute_relation_after_update` within `relvar-core/src/database/dml.rs` to destructure the relation instead of cloning the `relation_type` before iterating.

🎯 Why:
The `compute_relation_after_update` method previously cloned the entire `RelationType` before calling `into_iter()` on the `Relation`. Since `into_iter()` drops the original `RelationType` within `Relation`, cloning it was an unnecessary heap allocation.

📊 Impact:
Removes 1 unnecessary clone of `RelationType` (which contains a `TupleType` with a dynamically allocated `BTreeMap` of attributes) for every single execution of a DML UPDATE statement.

🔬 Measurement:
Run `cargo bench --bench tuple_creation` and the standard `cargo test` suite to verify tests pass and no performance regressions exist.
