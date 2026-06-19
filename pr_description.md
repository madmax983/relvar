🌟 Nova: Relational Garbage Collector

💡 **The Spark:** "Can we model complex memory management algorithms using pure relational operations? What if memory reachability is just a graph traversal problem?"
🚀 **The Feature:** "Implemented `RelationalGC` which uses a Mark-and-Sweep approach to identify unreachable nodes based on root relations, heap relations, and reference edges, powered purely by Relational Algebra operations like `tclose`, `join`, `union` and `difference`."
🔮 **The Potential:** "Could be expanded to perform actual tracing garbage collection within a relational memory space, or act as an interesting educational model for how GC concepts map to database operations."
⚠️ **Risk:** "Low. Isolated in `src/experimental/garbage_collector.rs` behind the `nova` feature flag."
