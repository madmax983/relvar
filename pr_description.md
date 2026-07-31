Title: 🌟 Nova: Relational Garbage Collector

💡 **The Spark:** I noticed we have powerful graph reachability operations like `tclose`, but we hadn't applied them to computer science fundamentals like memory management. Can we express Mark-and-Sweep garbage collection entirely declaratively?

🚀 **The Feature:** Implemented `GarbageCollector` in `src/experimental/garbage_collector.rs`. It models the heap, roots, and references as purely relational sets. The "Mark" phase computes live objects using `tclose` (transitive closure) and `join`, while the "Sweep" phase uses `difference` to precisely identify unreferenced garbage cycles and memory.

🔮 **The Potential:** Demonstrates that relational databases are naturally adept at complex topological algorithms like garbage collection without explicit recursion, opening the door for building language runtimes purely on top of relational engines.

⚠️ **Risk:** Extremely low. The feature is completely isolated in `src/experimental/` and purely additive.
