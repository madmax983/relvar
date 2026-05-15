**Bloat:** Query builder APIs (`Query::project`, `Query::rename`, `Query::summarize`) required passing owned `Vec<T>`, forcing callers to allocate intermediate vectors (`vec!["a", "b"]`) when they could just pass stack-allocated arrays (`["a", "b"]`).

**Cut:** Refactored the signatures of `project`, `rename`, and `summarize` in `relvar-core/src/query/mod.rs` to accept generic `IntoIterator<Item = T>` bounds instead. Replaced `vec![]` invocations with standard array literals `[]` across `Query` tests to prove out the simplified calling convention.

**Saved:** Heap allocation overhead for query builder operations, plus simplification and reduction of cognitive load across caller code, aligning perfectly with the KISS principle.
