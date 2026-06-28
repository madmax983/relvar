🌟 Nova: Relational Build System

💡 **The Spark:** "Build systems like Make or Ninja determine what targets to rebuild based on dependency graphs and file timestamps. Could we model this entirely in relational algebra?"
🚀 **The Feature:** "Implemented `RelationalBuildSystem` which represents targets and dependencies as relations, leveraging `tclose` to compute indirect dependencies, and joins with restrictions to identify stale targets."
🔮 **The Potential:** "Demonstrates that graph traversal for dependency resolution can be compiled into clean, declarative database queries."
⚠️ **Risk:** "Low. Purely additive and isolated in `src/experimental/build_system.rs`."
