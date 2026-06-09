🌟 Nova: Relational Turing Machine Simulator

💡 **The Spark:** I noticed that relational algebra is extremely powerful, but can it simulate universal computation purely through relation variables? Could we model a Turing Machine completely declaratively using tables and joins instead of procedural states and loops?

🚀 **The Feature:** Implemented `TuringMachine` structure and the `step` method. Modeled the `tape`, `head`, and `transitions` as purely Relational Algebra representations. Turing transitions are executed via join chains, projections, extensions and set operations, ensuring correct and iterative progression without imperative state updates. Added a test that runs a Turing Machine to invert the bits on the tape!

🔮 **The Potential:** Showcases the ultimate limit of relational operations: Turing-completeness! It means you can literally evaluate any computable function strictly with relational queries, serving as a huge validation of Relvar's expressive capabilities.

⚠️ **Risk:** Low. Isolated in `src/experimental/turing_machine.rs`. Only adds functionality to experimentations and modifies nothing of the core logic.
