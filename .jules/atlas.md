## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.

## 2024-03-XX - The Database Blob
**Tangle:** The `relvar-core/src/database/mod.rs` module grew into a 1,000-line God Module holding DDL, DML, constraint validation, transaction boundaries, and Virtual Relvars in a single implementation block.
**Blueprint:** Refactored `Database<E>` into a Facade pattern. The `mod.rs` file now orchestrates the public API while implementation details are cleanly separated into `schema.rs` (DDL), `data.rs` (DML/Querying), `integrity.rs` (Constraints), and `transaction.rs` (TCL) based on domain responsibility.

## 2026-04-11 - Module Coupling Refactor
**Tangle:** Several circular dependencies existed across modules: `types` <-> `values` in `relvar-core`, `mvcc` <-> `storage`, and `wal` <-> `storage` in `relvar-storage`.
**Blueprint:** Refactored dependencies by reallocating shared functionality. The `selector` method on `ScalarType` was replaced with `ScalarValue::select(&type, value)` mapping to correct responsibility since `ScalarValue` relies on `types`, thereby untangling the `relvar-core` loop. In `relvar-storage`, `collect_garbage` was relocated directly into the `storage::heap` namespace, eliminating the cyclical jump between `mvcc` and `storage`. Also decoupled `wal` <-> `storage` by replacing domain-specific struct pointers in `WalRecord` with primitive `u64` representing the page id, establishing unidirectional flow and solid boundaries.
## 2025-04-16 - [De-bloat storage modules]
**Tangle:** The `relvar-storage/src/storage/heap.rs` and `relvar-storage/src/persistent_engine.rs` modules were growing extremely large (4.5k+ and 1.8k+ lines respectively), causing a "Blob" anti-pattern due to having massive inline test modules.
**Blueprint:** Extracted the massive test modules into their own files (`relvar-storage/src/storage/heap/heap_tests.rs` and `relvar-storage/src/persistent_engine/persistent_engine_tests.rs`) to separate test code from core logic, greatly reducing the bloat and improving readability.
