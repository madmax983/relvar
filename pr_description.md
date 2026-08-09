🌟 Nova: [Relational Build System]

💡 **The Spark:** "Make/Ninja resolve dependencies by traversing a DAG. Can we just use relational transitive closures and natural joins instead?"
🚀 **The Feature:** "Implemented `find_stale_targets` in a new `build_system` module under `experimental`."
🔮 **The Potential:** "This showcases how a purely relational model can act as a fully declarative dependency resolver, computing out-of-date targets without explicit graph traversal."
⚠️ **Risk:** "Low. It is isolated in `src/experimental/build_system.rs`."
