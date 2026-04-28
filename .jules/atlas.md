## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.

## 2024-03-XX - The Database Blob
**Tangle:** The `relvar-core/src/database/mod.rs` module grew into a 1,000-line God Module holding DDL, DML, constraint validation, transaction boundaries, and Virtual Relvars in a single implementation block.
**Blueprint:** Refactored `Database<E>` into a Facade pattern. The `mod.rs` file now orchestrates the public API while implementation details are cleanly separated into `schema.rs` (DDL), `data.rs` (DML/Querying), `integrity.rs` (Constraints), and `transaction.rs` (TCL) based on domain responsibility.

## 2026-04-11 - Module Coupling Refactor
**Tangle:** Several circular dependencies existed across modules: `types` <-> `values` in `relvar-core`, `mvcc` <-> `storage`, and `wal` <-> `storage` in `relvar-storage`.
**Blueprint:** Refactored dependencies by reallocating shared functionality. The `selector` method on `ScalarType` was replaced with `ScalarValue::select(&type, value)` mapping to correct responsibility since `ScalarValue` relies on `types`, thereby untangling the `relvar-core` loop. In `relvar-storage`, `collect_garbage` was relocated directly into the `storage::heap` namespace, eliminating the cyclical jump between `mvcc` and `storage`. Also decoupled `wal` <-> `storage` by replacing domain-specific struct pointers in `WalRecord` with primitive `u64` representing the page id, establishing unidirectional flow and solid boundaries.
## 2024-05-18 - Extraction of Massive Files
**Tangle:** `relvar-storage/src/storage/heap.rs` and `relvar-storage/src/persistent_engine.rs` had become huge 'God Files' containing the struct definition, vast blocks of implementation code, and several thousand lines of inline tests each.
**Blueprint:** Used the `#[cfg(test)] mod tests;` idiom to safely extract tests into separate files within module directories (`storage/heap/tests.rs` and `persistent_engine/tests.rs`), significantly reducing the size of the implementation source files without changing semantics.
## 2024-05-19 - Sub-modularizing the Tests Blob
**Tangle:** The `relvar-storage/src/storage/heap/tests.rs` file had grown back into a 3,000+ line Blob despite being extracted from the main source file, making it extremely difficult to navigate and maintain.
**Blueprint:** Removed the `tests.rs` monolith and replaced it with a `tests/` directory structure containing specialized sub-modules (e.g., `insert.rs`, `scan.rs`, `update.rs`, `delete.rs`, `gc.rs`, `corruption.rs`, and `version.rs`) along with a `common.rs` file for shared helpers. This restores high cohesion to the testing structure.
## 2024-05-19 - Sub-modularizing the Persistent Engine Tests Blob
**Tangle:** The `relvar-storage/src/persistent_engine/tests.rs` file had grown into a massive Blob containing nearly 1,400 lines of inline tests for various aspects of the persistent engine (transactions, recovery, garbage collection, and basic operations).
**Blueprint:** Removed the `tests.rs` monolith and extracted its contents into a specialized sub-module structure under a new `relvar-storage/src/persistent_engine/tests/` directory. Created explicit sub-modules for `basic.rs`, `transaction.rs`, `recovery.rs`, and `gc.rs`, along with a `common.rs` for shared test initialization.
## 2024-05-19 - Sub-modularizing the Database Tests Blob
**Tangle:** The `relvar-core/src/database/tests.rs` file had grown into a 1,170 line Blob containing 45 inline tests. The God File mixed schema operations, DML, transaction boundaries, and virtual relvar tests.
**Blueprint:** Removed the `tests.rs` monolith and extracted its contents into a specialized sub-module structure under a new `relvar-core/src/database/tests/` directory. Created explicit sub-modules for `schema.rs`, `data.rs`, `integrity.rs`, `transaction.rs`, and `virtual_relvar.rs`, matching the core database domain responsibilities, along with a `common.rs` for shared helpers.
## 2024-05-19 - Sub-modularizing the Scan Tests Blob
**Tangle:** The `relvar-storage/src/storage/heap/tests/scan.rs` file had grown into a 1,229 line Blob despite being extracted from the main tests module, containing an amalgamation of visibility, corruption, security, and overflow tests, making it a "God File".
**Blueprint:** Extracted the file into smaller, more cohesive sub-modules: `scan_visible.rs`, `scan_corruptions.rs`, `scan_security.rs`, `scan_offset_overflow.rs`, and the remaining basic tests into `scan.rs`. Updated `relvar-storage/src/storage/heap/tests/mod.rs` to expose the new modules, resolving the structural bloat.
