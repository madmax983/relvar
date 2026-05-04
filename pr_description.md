💡 What: Optimized `compute_relation_after_update` in `relvar-core/src/database/dml.rs` to borrow `expected_type` as a reference rather than unnecessarily calling `.clone()` on the underlying `Arc<TupleType>`.
🎯 Why: Avoiding an unnecessary clone call on `Arc<TupleType>` during data modifications. This `Arc` clone was happening repeatedly during setup of the update operations and could easily be avoided by borrowing a reference instead.
📊 Impact: Minor speedup and removal of an unnecessary clone operation overhead, which could matter under heavy multi-threaded workloads where atomic refcounts are heavily contended. The `tuple_creation` bench showed a minor performance improvement.
🔬 Measurement: Run bench `tuple_creation` / `group_bench`.
