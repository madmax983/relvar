🪒 Razor: Flatten GraphNeuralNetwork namespace

**Bloat:** The `GraphNeuralNetwork` struct in `relvar/src/experimental/gnn.rs` was an empty namespace struct used merely to hold a single static `forward` method, acting as an unnecessary layer of abstraction.
**Cut:** Removed the `GraphNeuralNetwork` struct and its `impl` block, converting `forward` into a free module-level function.
**Saved:** Unnecessary namespace boilerplate and cognitive overhead.
