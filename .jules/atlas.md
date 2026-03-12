## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.

## 2024-05-19 - Removal of premature abstraction QueryExecutor
**Tangle:** The `QueryExecutor` trait in `relvar-core/src/traits.rs` was a premature abstraction created to decouple query evaluation from the concrete `Database` struct, even though only `Database` implements it, causing unnecessary indirection ("Enterprise" patterns).
**Blueprint:** Removed `QueryExecutor` entirely and replaced it with generic bounds on the concrete `Database<E>` struct where necessary (e.g., in `VirtualRelvarDefinition` and `Query::execute`), strengthening the contract, reducing trait pollution, and making the code more direct.
