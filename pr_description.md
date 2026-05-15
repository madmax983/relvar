🌟 Nova: Relational Quantum Circuit Simulator

💡 **The Spark:** I noticed we have modelled many physical systems and algorithms using relations, but nothing quantum yet. A quantum state is just a vector of complex amplitudes, and a gate is just a matrix. These are easily expressed as relations!

🚀 **The Feature:** Implemented `QuantumState` in a new `experimental::quantum` module. It represents quantum states as relations of probability amplitudes `(state_id: Int, real: Float, imag: Float)`. Quantum gates are represented as relations `(row_id: Int, col_id: Int, real: Float, imag: Float)`.
Gate application is cleanly modeled as a Relational Join (to align inputs to the gate matrix), Extension (to multiply complex numbers), and Summarization (to sum the intermediate amplitudes), effectively performing matrix multiplication via database queries. Multi-qubit gates are computed via a Relational Kronecker Product (using cross joins).
I also added helper functions to build Identity and Hadamard gates!

🔭 **The Potential:** This proves that our relational database engine can simulate quantum mechanics purely through declarative queries. It opens the door for writing quantum algorithms (like Grover's or Shor's) using relational algebra, demonstrating the extreme versatility of Relvar.

⚠️ **Risk:** Low. Completely isolated within `src/experimental/quantum.rs` and exposed via the `experimental` module. Does not touch or modify any core query execution logic.
