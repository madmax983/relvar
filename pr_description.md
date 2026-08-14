# 🌟 Nova: Relational CRDT (OR-Set)

💡 **The Spark:** I noticed we have implemented many complex systems but haven't explored distributed data structures yet. Could we implement Conflict-free Replicated Data Types (CRDTs) using purely relational algebra?

🚀 **The Feature:** Implemented `OrSet` (Observed-Remove Set) purely in relational algebra. It uses separate relations for `adds` and `removes` tagged with unique identifiers.

🔭 **The Potential:** Enables building distributed database features, collaborative editing systems, or any eventually-consistent application natively within the relational engine.

⚠️ **Risk:** Low. The feature is completely isolated within the `src/experimental/crdt.rs` module and does not impact existing core logic.
