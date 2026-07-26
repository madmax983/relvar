Title: 🌟 Nova: Relational Garbage Collector

💡 **The Spark:** I realized that Garbage Collection algorithms map perfectly to graph reachability problems, which can be solved declaratively using relational algebra!

🚀 **The Feature:** Implemented `RelationalGC`, a Mark-and-Sweep Garbage Collector that runs entirely on relational algebra!
- **Heap**, **Roots**, and **References** are modeled as pure relations.
- **Mark Phase:** Computes the reachable object set using `tclose` (transitive closure) on references, joined with the roots.
- **Sweep Phase:** Identifies garbage using a pure relational `difference` between the total heap and the reachable set.

🔮 **The Potential:** Demonstrates how relational engines can manage complex graphs and state evaluation declaratively. This could be used for dependency resolution, memory analysis, or as a teaching tool for GC algorithms without writing procedural traversal logic.

⚠️ **Risk:** Low. The feature is entirely self-contained within `src/experimental/garbage_collector.rs` and gated behind the `nova` feature flag. It doesn't modify any core logic.
