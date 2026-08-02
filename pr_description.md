Title: 🌟 Nova: Relational Build System

💡 **The Spark:** Build systems like Make or Ninja compute dependencies and compare timestamps to figure out what needs to be rebuilt. I noticed we can compute transitive closures and compare values in relations, so we could theoretically implement a build system natively in a relational database engine.
🚀 **The Feature:** Implemented `find_stale_targets` in `experimental/build_system.rs`. It evaluates dependencies purely in relational algebra using `tclose`, `join`, and `restrict` to figure out out-of-date build targets!
🔮 **The Potential:** Could be used to evaluate dynamic, recursive queries or power an actual relational task runner in the future.
⚠️ **Risk:** Low. Isolated in `src/experimental/build_system.rs` behind the `nova` feature flag.
