# 🌟 Nova: Relational Build System

💡 **The Spark:** "Can we model dependencies and file timestamps purely as relations, and compute stale build targets declaratively without writing recursive tree-traversal algorithms?"

🚀 **The Feature:** "Implemented `BuildSystem` using purely relational algebra. It models dependencies and modified times as relations, using Transitive Closure (`tclose`), Joins, and Difference to automatically resolve stale targets."

🔭 **The Potential:** "Demonstrates that a pure relational database engine can operate as a native build system and task orchestrator, declaratively evaluating complex data pipelines!"

⚠️ **Risk:** "Low. Isolated in `src/experimental/build_system.rs` behind the 'nova' feature flag."
