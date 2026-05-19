🌟 Nova: Relational Quantum Simulator

💡 **The Spark:** "I noticed we can model linear algebra and complex logic circuits using relational algebra, but we haven't touched quantum computing. Can we map quantum states to relations and simulate quantum interference declaratively?"

🚀 **The Feature:** "Implemented `QuantumSimulator` in `experimental/quantum.rs`. It models a sparse quantum state vector as a relation `(basis, amplitude_real, amplitude_imag)`. Single qubit gates are applied via `Join` with a gate matrix relation, and quantum interference is naturally computed via `Summarize` (aggregation). Supports Hadamard, Pauli-X, and CNOT gates, enabling Bell state entanglement."

🔮 **The Potential:** "Could be used to build a fully declarative quantum circuit transpiler or analyzer within the database engine, exploring large-scale quantum simulations without procedural arrays."

⚠️ **Risk:** "Low. Entirely additive and isolated in `src/experimental/quantum.rs`."
