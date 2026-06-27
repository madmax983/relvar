🌟 Nova: Relational Turing Machine

💡 **The Spark:** Relational engines evaluate queries declaratively, but can they compute *anything*? We have cellular automata and expert systems, but implementing a Turing Machine proves definitively that pure relational algebra (with a loop) is Turing-complete!

🚀 **The Feature:** Implemented `RelationalTuringMachine` in `src/experimental/turing_machine.rs`. It represents the machine's state (transitions, tape, head position) purely as standard relations. A single "step" of the machine is evaluated completely declaratively via Joins, Semidifferences, Extensions, and Unions, mapping the current state to the next state. It is shipped with a working 2-state Busy Beaver test.

🔭 **The Potential:** This theoretically proves that any algorithm, no matter how complex, can be decomposed into native relational operations. It also opens up paths to evaluate procedural stored procedures strictly as relational queries!

⚠️ **Risk:** Low. Completely isolated in `src/experimental/turing_machine.rs` and has no impact on the core engine's existing capabilities.
