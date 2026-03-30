## [Reduction]
**Bloat:** `QueryAggregation` struct duplicating `Aggregation` just for serialization. `Query::execute` taking `&mut` unnecessarily. Core query logic in `experimental`.
**Cut:** Removed `QueryAggregation`, moved `Query` to core, relaxed `execute` mutability.
**Saved:** ~50 lines of duplicate code, improved API ergonomics, better project structure.

## [Reduction]
**Bloat:** `RelationSource` trait. It was a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removed `RelationSource` trait and updated `Query::execute` to depend directly on `Database`.
**Saved:** Removed 1 file (`traits.rs`), removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.

## [Reduction]
**Bloat:** `QueryExecutor` trait. It is a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removing `QueryExecutor` trait and updating `Query::execute` and virtual relvars to depend directly on `Database`.
**Saved:** Removed `traits.rs`, removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.
## [Reduction]
**Bloat:** The `Pivot` trait in `relvar/src/experimental/pivot.rs` was a single-implementation trait applied only to `Relation`.
**Cut:** Removed the `Pivot` trait and its `impl` block, converting it into a concrete, standalone function `pub fn pivot(relation: &Relation, ...)`.
**Saved:** Reduced boilerplate, flattened abstraction, simplified method resolution, and removed a "One-Time Trait."
## [Reduction]
**Bloat:** `SlotDescriptor` and `MutableSlot` traits in `relvar-storage/src/storage/heap.rs`. These were "One-Time Traits" implemented only by 2 concrete structs (`SlotEntry` and `VersionedSlotEntry`) to share tiny bits of logic. This is an unnecessary abstraction for "Future Proofing".
**Cut:** Removed the traits entirely. Implemented `offset`, `length`, `set_offset`, and `set_length` explicitly on both structs. Duplicated the implementation logic for `repack_slots`, `extract_all_tuples`, and `extract_tuples_from_slots` for the two concrete types.
**Saved:** Removed 2 traits, 2 generic trait bounds, simplified method resolution, reduced abstraction layer cognitive load.
## [Reduction]
**Bloat:** Deep folder hierarchies for `query` and `storage_engine` modules which only contained 1 and 2 files respectively.
**Cut:** Flattened `relvar-core/src/query/mod.rs` to `relvar-core/src/query.rs` and merged `relvar-core/src/storage_engine/in_memory.rs` into `relvar-core/src/storage_engine.rs`, deleting the directories.
**Saved:** Removed 2 directories and 1 file, significantly reducing navigation overhead and matching file count to directory depth.

## [Reduction]
**Bloat:** Single-variant `Error` enums (`UnionError`, `IntersectError`, `DifferenceError`) that added unnecessary pattern matching boilerplate and indirection.
**Cut:** Replaced the 1-variant enums with concrete `thiserror` unit structs (e.g., `pub struct UnionError;`).
**Saved:** Removed ~10 lines of enum boilerplate and simplified error handling at the call sites.
