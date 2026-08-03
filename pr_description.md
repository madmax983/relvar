Title: 🌟 Nova: [Relational Build System]

💡 **The Spark:** Build systems (like Make/Ninja) often calculate a target's dependencies to decide whether a rebuild is needed. What if we model this dependency graph and the file metadata simply as relations in our system and perform the rebuild checks purely declaratively using relational algebra?

🚀 **The Feature:** Implemented `BuildSystem` in `relvar_core::experimental::build_system`. It takes a `dependencies` relation (edges of the target dependency graph) and `file_times` (file mtimes) and discovers stale targets declaratively via `tclose`, relational joins, and restrictions, compiling a core build algorithm into purely set operations!

🔮 **The Potential:** Validates that relational databases can double as task runners! We can declaratively compute build dependencies and track file modification timestamps to efficiently orchestrate builds.

⚠️ **Risk:** Low. The algorithm is safely isolated in `src/experimental/build_system.rs` behind the `nova` feature flag and thoroughly tested.
