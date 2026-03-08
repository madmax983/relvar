## [Reduction]
**Bloat:** `QueryAggregation` struct duplicating `Aggregation` just for serialization. `Query::execute` taking `&mut` unnecessarily. Core query logic in `experimental`.
**Cut:** Removed `QueryAggregation`, moved `Query` to core, relaxed `execute` mutability.
**Saved:** ~50 lines of duplicate code, improved API ergonomics, better project structure.

## [Reduction]
**Bloat:** `RelationSource` trait. It was a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removed `RelationSource` trait and updated `Query::execute` to depend directly on `Database`.
**Saved:** Removed 1 file (`traits.rs`), removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.

## [Reduction]
 **Bloat:** `QueryExecutor` trait in `relvar-core/src/traits.rs`. It was a "One-Time Trait" implemented exclusively by `Database<E>`, adding unnecessary abstraction and indirection.
 **Cut:** Removed `QueryExecutor` trait, replaced its usages with the concrete `Database<E>` structure.
 **Saved:** Removed `traits.rs` file, simplified generic bounds in `query` and `search` logic, and deleted ~30 LOC of boilerplate trait definitions and implementations.## [Reduction]
**Bloat:** `SlotDescriptor` and `MutableSlot` traits in `heap.rs`. They were only implemented by `SlotEntry` and `VersionedSlotEntry`, needlessly genericizing page mutations and adding abstraction layers.
**Cut:** Removed the traits entirely. Replaced `repack_slots<T: MutableSlot>` with two small concrete implementations (`repack_slots` and `repack_versioned_slots`). Updated tuple extraction methods to directly take iterators of raw `(u32, u32)` offset/length coordinates rather than dynamic struct properties.
**Saved:** ~50 lines of boilerplate trait code, reduced cognitive overhead, clearer data-flow.

## [Reduction]
**Bloat:** `Pivot` extension trait in `relvar/src/experimental/pivot.rs`. It was a "One-Time Trait" strictly implemented by `Relation`, requiring users to import the trait just to use the function.
**Cut:** Deleted the trait and refactored the method into a standalone `pub fn pivot(...)` taking `&Relation` directly.
**Saved:** 15 lines of boilerplate, vastly simpler API surface, eliminated "magic" extension method behavior.
