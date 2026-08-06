Title: 🌟 Nova: Relational Quantum Circuit Simulator

💡 **The Spark:** Quantum states and tensor networks can be viewed as relational tensors. Can we compute quantum state vector evolution using pure relational joins?
🚀 **The Feature:** Implemented `QuantumCircuit` to simulate qubit states natively inside the relational engine, translating 1-qubit and 2-qubit gates into natural joins, mapped multiplications via extension, and summarizations.
🔮 **The Potential:** Demonstrates extreme expressiveness of relational algebra—proving that relational queries can implement tensor contraction operations for quantum simulation!
⚠️ **Risk:** Low. The changes are completely isolated within the new module `relvar/src/experimental/quantum.rs`.
