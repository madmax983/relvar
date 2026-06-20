🌟 Nova: Relational CRDTs (OR-Set)

💡 **The Spark:** I noticed that we have many graph, string, and numeric experimental algorithms but lack robust distributed data structures. Can we model an asynchronous distributed data type—like an Observed-Remove Set (OR-Set)—entirely inside the relational boundaries without external synchronization logic?

🚀 **The Feature:** Implemented `OrSet` CRDT inside `src/experimental/crdt.rs`. It represents `adds` and `removes` operations as unique sets of elements and tag relations. The active elements are derived strictly from relational `Difference` and `Project`, and concurrent merges are evaluated purely via relational `Union`.

🔭 **The Potential:** Provides a foundation for declarative conflict-resolution directly within the database queries! Relational Unions automatically deduce state convergence logic, removing the need to manage distributed conflicts manually.

⚠️ **Risk:** Low. Isolated in `src/experimental/crdt.rs`. It only depends on the core Relation primitives (Union, Difference, Project).
