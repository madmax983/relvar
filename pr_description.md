🌟 Nova: Relational Build System

💡 **The Spark:** "Can we model a Build System purely with relational algebra?" Build systems like Make or Ninja determine target staleness based on dependencies. By using relational representations, can we evaluate build graphs using nothing but Set Theory?

🚀 **The Feature:** "Implemented `BuildSystem` in `relvar-core/src/experimental`." This module creates `files` and `dependencies` relations. Targets are marked as stale through relational joins, transitive closure (`tclose`) on the dependency edges, and aggregations mapping target files to the maximum timestamp of their cascading dependencies.

🔮 **The Potential:** "Demonstrates how dependency resolution, usually implemented as procedural graph traversal, compiles elegantly into Declarative Relational Algebra queries!"

⚠️ **Risk:** "Low. Isolated in `src/experimental/build_system.rs` behind the `nova` feature flag."
