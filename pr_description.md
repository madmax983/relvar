🪒 Razor: Flatten GraphNeuralNetwork namespace

**Bloat:** The `GraphNeuralNetwork` unit struct in `relvar/src/experimental/gnn.rs` acting solely as an empty namespace for a single static method `forward`.

**Cut:** Removed the struct and its `impl` block, converting `forward` into a standalone module-level function.

**Saved:** Unnecessary struct definition and indentation, embracing idiomatic Rust module-level functions over OOP-style static classes.
