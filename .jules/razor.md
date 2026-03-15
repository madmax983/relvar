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
