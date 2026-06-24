🌟 Nova: Relational Build System Simulator

💡 **The Spark:** Build systems traditionally resolve dependencies via procedural graph transversals. Since we have a relational engine, can we declaratively compile stale targets using relational queries over file dependencies and timestamps?

🚀 **The Feature:** Implemented `compute_stale_targets` in `relvar-core/src/experimental/build_system.rs`. It evaluates dependencies mapping via transitive closure (`tclose`), maps file timestamps against targets via joins, and evaluates stale conditions via set differences and restrictions.

🔮 **The Potential:** This expands the horizons of relational algebra proving that complex constraint satisfaction (like Make/Ninja target compilation) maps beautifully into declarative SQL-like data transformations. It demonstrates that build logic can reside fully inside a database!

⚠️ **Risk:** Low. The algorithm runs purely inside an isolated experimental module (`build_system`) using existing relvar-core primitives without modifying any core runtime logic.
