🪒 Razor: [KISS simplifications]

## [Reduction]
**Bloat:** Unnecessary struct `GraphNeuralNetwork` used solely to hold a stateless `forward` function in `relvar/src/experimental/gnn.rs` (a 'One-Time Trait'/'Stateless Class' anti-pattern).
**Cut:** Deleted the `pub struct GraphNeuralNetwork;` and its `impl` block, extracting `pub fn forward` directly as a standalone module-level function.
**Saved:** Unnecessary instantiation boilerplate and indentation level.

## [Reduction]
**Bloat:** Over-complex return type `Result<Option<Relation>, DatabaseError>` in `relvar/src/experimental/automata.rs`.
**Cut:** Flattened the return type to `Result<Relation, DatabaseError>`, returning empty instances (empty `Relation`) instead of `None` to avoid double-unwrapping.
**Saved:** Several layers of nested matching and reduced cognitive complexity for callers.
