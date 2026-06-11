🌟 Nova: Relational Garbage Collector

💡 **The Spark:** "I noticed we have `tclose` for graphs, but we haven't applied it to system level algorithms like garbage collection."
🚀 **The Feature:** "Implemented `GarbageCollector` that computes reachability using `tclose` and finds unreferenced memory via relational `difference`."
🔭 **The Potential:** "Demonstrates that relational engines can natively model and solve mark-and-sweep garbage collection declaratively!"
⚠️ **Risk:** "Low. The feature is entirely self-contained within `src/experimental/garbage_collector.rs`."
