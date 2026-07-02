🌟 Nova: [Relational Build System]

💡 **The Spark:** I noticed we can compute transitive closures and aggregations, which is basically what Ninja and Make do to figure out what files are out of date! Can we find stale build targets purely declaratively?

🚀 **The Feature:** Implemented `find_stale_targets` in `src/experimental/build_system.rs`. It models dependencies as relations, uses `tclose` to find all transitive dependencies, and then joins and summarizes with timestamps to find targets older than their dependencies.

🔮 **The Potential:** Turns a relational database into a native dependency solver. Could be the foundation for an entirely new kind of declarative build system stored in Postgres or Relvar.

⚠️ **Risk:** Low. Isolated purely additive implementation in `src/experimental/build_system.rs`.
