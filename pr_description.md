🌟 Nova: Relational Garbage Collector
💡 **The Spark:** I noticed that we have `tclose` (transitive closure) and `difference` operators that could perfectly capture the concept of garbage collection via a Mark-and-Sweep algorithm, all purely evaluated via relational queries!
🚀 **The Feature:** Implemented `mark_and_sweep` in `garbage_collector.rs` which takes `roots`, `heap`, and `references` relations and declaratively resolves which memory allocations are unreachable and should be collected.
🔭 **The Potential:** Provides a working blueprint for resolving generic graph reachability algorithms directly in the database engine without procedural graph traversals.
⚠️ **Risk:** Low. Fully isolated in `src/experimental/garbage_collector.rs` under the `nova` feature flag.
