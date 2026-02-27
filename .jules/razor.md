## [Reduction]
**Bloat:** `QueryAggregation` struct duplicating `Aggregation` just for serialization. `Query::execute` taking `&mut` unnecessarily. Core query logic in `experimental`.
**Cut:** Removed `QueryAggregation`, moved `Query` to core, relaxed `execute` mutability.
**Saved:** ~50 lines of duplicate code, improved API ergonomics, better project structure.

## [Reduction]
**Bloat:** `RelationSource` trait. It was a "One-Time Trait" implemented only by `Database`, adding unnecessary abstraction and indirection for speculative "Future Proofing".
**Cut:** Removed `RelationSource` trait and updated `Query::execute` to depend directly on `Database`.
**Saved:** Removed 1 file (`traits.rs`), removed ~20 lines of code, reduced cognitive load by removing an unnecessary abstraction layer.

## [Reduction]
**Bloat:** `QueryExecutor` trait. Another "One-Time Trait" that was only implemented by `Database`, adding indirection.
**Cut:** Removed `QueryExecutor` completely from `relvar-core/src/traits.rs` and the library exports. Updated `Query::execute` and `VirtualRelvarDefinition` to take `&Database<E>` directly, flattening the module structure.
**Saved:** Deleted `traits.rs` and `virtual_relvar.rs`, saved lines of code, reduced abstraction boilerplate significantly.
