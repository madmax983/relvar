# 🌟 Nova: Relational Turing Machine Simulator

💡 **The Spark:** "I wonder if we can prove the Relational Algebra implementation in Relvar is Turing Complete by simulating a Turing Machine directly inside it?"

🚀 **The Feature:** Implemented `turing_machine` simulator in `relvar-core/src/experimental/turing_machine.rs`. It evaluates a single transition step of a Turing Machine using purely relational operations (Join, Restrict, Project, Rename, Union, Extend, Difference) over Tape, Head, and Transitions relations. Added a test for the Busy Beaver 2-state 2-symbol (BB-2) algorithm.

🔭 **The Potential:** By proving universal computation is possible via pure declarative relational operators, it reinforces the immense power of the relational model. Future modules could even execute entire programs defined as relations!

⚠️ **Risk:** Low. Isolated in `src/experimental/turing_machine.rs` behind the `nova` feature flag. Uses standard tested relational algebra operations internally.
