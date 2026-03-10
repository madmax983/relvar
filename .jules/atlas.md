## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.
