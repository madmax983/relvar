## 2024-03-XX - Module Encapsulation Cleanup
**Tangle:** Broad visibility (`pub mod`) across many internal modules (`algebra`, `values`, `types`, etc.) leaked implementation details and complicated the dependency graph.
**Blueprint:** Converted most top-level internal module definitions in `relvar-core`, `relvar-storage`, and `relvar` to `pub(crate) mod`. Re-exported necessary types via their respective `mod.rs` files, ensuring clean, intention-revealing public APIs while maintaining low coupling between internal components.

## 2024-03-XX - The Database Blob
**Tangle:** The `relvar-core/src/database/mod.rs` module grew into a 1,000-line God Module holding DDL, DML, constraint validation, transaction boundaries, and Virtual Relvars in a single implementation block.
**Blueprint:** Refactored `Database<E>` into a Facade pattern. The `mod.rs` file now orchestrates the public API while implementation details are cleanly separated into `schema.rs` (DDL), `data.rs` (DML/Querying), `integrity.rs` (Constraints), and `transaction.rs` (TCL) based on domain responsibility.

## 2024-04-12 - Resolving Circular Dependencies in Storage and Core
**Tangle:**
1. Circular dependency between `relvar-storage/src/mvcc` and `relvar-storage/src/storage`: `mvcc` depended on `storage` for `HeapFile` to do garbage collection, and `storage::heap` depended on `mvcc::TransactionSnapshot` for visibility.
2. Circular dependency between `relvar-core/src/types` and `relvar-core/src/values`: `ScalarType` had a `selector` method that operated on `ScalarValue`, requiring `types` to depend on `values` while `values` naturally depended on `types`.
**Blueprint:**
1. Moved the `collect_garbage` logic (and its associated tests) entirely into `relvar-storage/src/storage/heap.rs` where the actual internal slot scanning logic resided. Deleted `mvcc/gc.rs` and the `pub(crate) mod gc` declaration to enforce a strict unidirectional dependency where `storage` understands `mvcc` boundaries, but `mvcc` focuses purely on abstract concurrency tokens.
2. Relocated the `selector` logic from `types/scalar.rs` to an associated function `ScalarValue::select(ty, value)` in `values/scalar.rs`. Updated all call sites in tests, parsers (importer/exporter), and documentation to reflect this structural correction. This ensures that `types` module has zero dependencies on runtime `values` module.
