## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.

## 2024-03-XX - The Database Blob
**Tangle:** The `relvar-core/src/database/mod.rs` module grew into a 1,000-line God Module holding DDL, DML, constraint validation, transaction boundaries, and Virtual Relvars in a single implementation block.
**Blueprint:** Refactored `Database<E>` into a Facade pattern. The `mod.rs` file now orchestrates the public API while implementation details are cleanly separated into `schema.rs` (DDL), `data.rs` (DML/Querying), `integrity.rs` (Constraints), and `transaction.rs` (TCL) based on domain responsibility.

## 2024-03-31 - The Query Blob
**Tangle:** The `relvar-core/src/query/mod.rs` file became a Blob, containing AST, execution logic, builder patterns, and tests in one single 600+ line file. This violated separation of concerns and resulted in a monolithic module.
**Blueprint:** Split the module into `ast.rs` (Data Structures), `builder.rs` (Fluent API methods), `execute.rs` (Execution/Explain logic), and `tests.rs` (Unit tests), hiding them behind `mod.rs` acting as a Facade. This cleanly separates concerns into independent files while keeping a simple API.
