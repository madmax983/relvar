💡 **The Spark:** I noticed we have transitive closures and relational differences. What if we could execute mark-and-sweep garbage collection cleanly?
🚀 **The Feature:** Implemented `mark_and_sweep` in `experimental/garbage_collector.rs` purely via relational algebra (tclose, join, union, difference).
🔮 **The Potential:** Enables modeling complex runtime engines directly over Relvar without writing procedural traversal logic.
⚠️ **Risk:** Low. Isolated in `src/experimental/`.
