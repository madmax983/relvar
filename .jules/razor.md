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
