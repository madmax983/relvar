## [Reduction]
**Bloat:** `QueryAggregation` struct duplicating `Aggregation` just for serialization. `Query::execute` taking `&mut` unnecessarily. Core query logic in `experimental`.
**Cut:** Removed `QueryAggregation`, moved `Query` to core, relaxed `execute` mutability.
**Saved:** ~50 lines of duplicate code, improved API ergonomics, better project structure.

## [Reduction]
**Bloat:** `RelationSource` trait. It was a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removed `RelationSource` trait and updated `Query::execute` to depend directly on `Database`.
**Saved:** Removed 1 file (`traits.rs`), removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.

## [Reduction]
**Bloat:** `QueryExecutor` single-implementation trait, `Pivot` single-implementation trait, `SlotDescriptor` and `MutableSlot` single-use traits.
**Cut:** Replaced `QueryExecutor` with concrete generic type `Database<E>`, replaced `Pivot` with free function `pivot`, replaced `SlotDescriptor` and `MutableSlot` traits with macros and iterator mapping in `heap.rs`.
**Saved:** Eliminated 3 traits, removing multiple files and over 100 lines of boilerplate, resulting in more direct, readable, concrete code.
