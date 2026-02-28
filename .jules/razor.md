## [Reduction]
**Bloat:** `QueryAggregation` struct duplicating `Aggregation` just for serialization. `Query::execute` taking `&mut` unnecessarily. Core query logic in `experimental`.
**Cut:** Removed `QueryAggregation`, moved `Query` to core, relaxed `execute` mutability.
**Saved:** ~50 lines of duplicate code, improved API ergonomics, better project structure.

## [Reduction]
**Bloat:** `RelationSource` trait. It was a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removed `RelationSource` trait and updated `Query::execute` to depend directly on `Database`.
**Saved:** Removed 1 file (`traits.rs`), removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.

## [Reduction]
**Bloat:** `QueryExecutor` trait. It was another "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection.
**Cut:** Removed `QueryExecutor` trait, deleted `traits.rs`, and updated `Query::execute` and `VirtualRelvarDefinition` to depend directly on `Database<E>`.
**Saved:** Removed 1 file (`traits.rs`), deleted `virtual_relvar.rs` by merging it into `mod.rs`, removed ~25 lines of code, simplified the API and reduced cognitive load by removing an unnecessary abstraction layer.
