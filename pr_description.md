🌟 Nova: Relational Turing Machine Simulator

💡 **The Spark:** We've implemented graphs, state machines, and spread sheets using pure relational algebra. But can we prove the engine is theoretically capable of arbitrary computation? Can we emulate a Universal Turing Machine?

🚀 **The Feature:** Implemented `turing_machine_step` and `run_turing_machine` purely using relational operations. The tape, transition rules, and head state are modeled as relation tables. State transitions and tape updates are computed using declarative joins, extensions, set differences, and unions.

🔮 **The Potential:** Demonstrates Turing-completeness of the Relvar algebra! This acts as an ultimate stress test for the algebraic primitives and opens the door to interpreting other computational models natively within the database engine without procedural loops.

⚠️ **Risk:** Low. Completely isolated in `src/experimental/turing_machine.rs` and has comprehensive unit tests covering halting logic and infinite tape expansion.
