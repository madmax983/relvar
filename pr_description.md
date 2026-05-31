💡 What: Replaced calls to `restrict` with `restrict_into` where the caller owned the `Relation`.
🎯 Why: `restrict` allocates a new `Relation` and `HashSet`, cloning tuples. If the caller already owns the `Relation`, `restrict_into` mutates it in-place with `HashSet::retain`, entirely removing allocations and clones. This was identified in `experimental/knowledge_graph.rs`, `database/schema.rs`, `experimental/game_of_life.rs` and `lib.rs`
📊 Impact: Reduces heap allocations and completely removes tuple cloning per filter operation on owned relations.
🔬 Measurement: Run `cargo bench`, verify compile and tests pass (`cargo test`).
