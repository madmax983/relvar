As per Razor's philosophy to remove complexity, speculative generality, and dead code, this PR makes the following targeted simplifications to enforce the KISS principle:

1. **Flattening Hierarchy**: Replaced the deep folder hierarchy in `relvar-core/src/query/tests/` (which only contained 3 small files) into a single, flat `relvar-core/src/query/tests.rs` module, reducing unnecessary directory nesting.
2. **Deleting Zombie Code**: Deleted `are_satisfied_by` and `would_violate_on_insert` functions from `KeyConstraints` in `relvar-core/src/constraints/key.rs` (along with their tests) as they were completely unused by the `ConstraintManager`, which invoked these checks directly on `PrimaryKey` and `CandidateKey`.
