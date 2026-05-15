🌟 Nova: Relational Garbage Collector

💡 **The Spark:** "Can we model complex algorithms like Garbage Collection using purely declarative relational queries without any procedural graph traversal logic?"

🚀 **The Feature:** "Implemented `find_garbage` inside `relvar-core/src/experimental/garbage_collector.rs` which evaluates a Mark-and-Sweep Garbage Collector purely via Relational Algebra. Represents `roots`, `heap`, and `references` as relations. The Mark phase computes reachability via `tclose`, `join`, and `union`. The Sweep phase identifies garbage via `difference`."

🔮 **The Potential:** "This powerfully demonstrates the expressive power of relational queries, seamlessly expressing a complex and commonly procedural memory-management problem via clean declarative query patterns."

⚠️ **Risk:** "Low. Isolated under `src/experimental/garbage_collector.rs` with no dependencies on core relational logic or database storage engines."