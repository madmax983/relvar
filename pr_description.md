# 🌟 Nova: [Relational Build System]

💡 **The Spark:** "Build systems like Make/Ninja use complex graph traversals to find stale targets. Can a relational engine do it natively using only relations?"

🚀 **The Feature:** "Implemented `find_stale_targets` in `src/experimental/build_system.rs` using purely relational algebra. It leverages `tclose` to resolve indirect dependencies and `join`/`restrict` to check modification times, avoiding manual tree traversal."

🔭 **The Potential:** "Demonstrates that relational engines can natively resolve build graphs and dependency trees declaratively."

⚠️ **Risk:** "Low. Isolated in `src/experimental/build_system.rs`."
