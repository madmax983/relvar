## [Reduction]
**Bloat:** `QueryAggregation` struct duplicating `Aggregation` just for serialization. `Query::execute` taking `&mut` unnecessarily. Core query logic in `experimental`.
**Cut:** Removed `QueryAggregation`, moved `Query` to core, relaxed `execute` mutability.
**Saved:** ~50 lines of duplicate code, improved API ergonomics, better project structure.

## [Reduction]
**Bloat:** `RelationSource` trait. It was a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removed `RelationSource` trait and updated `Query::execute` to depend directly on `Database`.
**Saved:** Removed 1 file (`traits.rs`), removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.
## [Reduction]
**Bloat:** `QueryExecutor` trait, `Pivot` trait, `SlotDescriptor`, `MutableSlot`. One-Time Traits implemented by only one object.
**Cut:** Replaced `QueryExecutor` with concrete `Database<E>`, turned `Pivot` into a free function, manually duplicated small slot helpers.
**Saved:** Removed 1 file (`traits.rs`), simplified generics, improved code locality.
