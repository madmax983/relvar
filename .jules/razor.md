## [Reduction]
**Bloat:** Single-variant enums `UnionError`, `IntersectError`, and `DifferenceError` in `relvar-core/src/algebra/` acting as unit structs but declared as enums.
**Cut:** Converted them to unit structs (`pub struct UnionError;`, etc.) to simplify error handling and reduce boilerplate, aligning with the KISS principle.
**Saved:** Unnecessary enum matching and boilerplate code.

## [Reduction]
**Bloat:** Single-variant enums `UnionError`, `IntersectError`, and `DifferenceError` in `relvar-core/src/algebra/` acting as unit structs but declared as enums.
**Cut:** Converted them to unit structs (`pub struct UnionError;`, etc.) to simplify error handling and reduce boilerplate, aligning with the KISS principle.
**Saved:** Unnecessary enum matching and boilerplate code.

## [Reduction]
**Bloat:** Single-variant enum `ScalarValueError` with a single variant `NotUserDefined` acting as a unit error.
**Cut:** Converted to unit struct `pub struct ScalarValueError;` for consistency and to reduce boilerplate and nested code matching.
**Saved:** Unnecessary enum matching for single error condition, simpler `return Err(ScalarValueError);` rather than `return Err(ScalarValueError::NotUserDefined);`

## [Reduction]
**Bloat:** Single-variant enum `ScalarTypeError` with a single variant `TypeMismatch` acting as a unit error struct but containing fields, in `relvar-core/src/types/scalar.rs`.
**Cut:** Converted to struct `pub struct ScalarTypeError;` with fields for expected and actual types for consistency and to reduce boilerplate and nested code matching, aligning with the KISS principle.
**Saved:** Unnecessary enum matching for a single error condition.
## [Reduction]
**Bloat:** Single-variant enum `RelationError` in `relvar-core/src/values/relation.rs` acting as a unit struct, with an unused variant `DuplicateTuple` and only one used variant `TypeMismatch`.
**Cut:** Converted to unit struct `pub struct RelationError;` with `#[error("Tuple does not conform to relation type")]` directly. Removed the unused `DuplicateTuple` variant.
**Saved:** Unnecessary enum matching, simplified error handling code significantly.

## [Reduction]
**Bloat:** Query builder APIs (`Query::project`, `Query::rename`, `Query::summarize`) required passing owned `Vec<T>`, forcing callers to allocate intermediate vectors (`vec!["a", "b"]`) when they could just pass stack-allocated arrays (`["a", "b"]`).
**Cut:** Refactored the signatures of `project`, `rename`, and `summarize` in `relvar-core/src/query/mod.rs` to accept generic `IntoIterator<Item = T>` bounds instead. Replaced `vec![]` invocations with standard array literals `[]` across `Query` tests to prove out the simplified calling convention.
**Saved:** Heap allocation overhead for query builder operations, plus simplification and reduction of cognitive load across caller code, aligning perfectly with the KISS principle.
