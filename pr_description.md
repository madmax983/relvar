🪒 Razor: Remove empty namespace struct GraphNeuralNetwork

**Bloat:** Empty namespace struct `GraphNeuralNetwork` acting as an unnecessary wrapper for a single function in `gnn.rs`.
**Cut:** Replaced the struct and its `impl` block with a single module-level free function `forward`. Fixed module visibilities causing unused item warnings.
**Saved:** Unnecessary indentation, a useless struct declaration, and verbose call paths (`GraphNeuralNetwork::forward`).
