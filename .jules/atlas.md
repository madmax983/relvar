## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.
## 2025-03-16 - Splitting the Database Blob
**Tangle:** The `Database` struct implementation in `relvar-core/src/database/mod.rs` had grown into a ~1,000 line "Blob" anti-pattern containing all DDL, DML, transaction, and integrity logic, violating high cohesion and making the module difficult to navigate.
**Blueprint:** Refactored the `Database` implementation by splitting it into four highly cohesive submodules: `schema.rs` (DDL), `data.rs` (DML), `integrity.rs` (Constraints), and `transaction.rs` (ACID transactions). The `mod.rs` now acts as a clean Facade, re-exporting only what is necessary and exposing `pub(crate)` helper methods for its submodules.
