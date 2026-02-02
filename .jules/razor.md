## [Reduction]
**Bloat:** Single-implementation extension traits (`ExtendOps`, `GroupOps`, `SummarizeOps`) for `Relation`.
**Cut:** Moved methods directly to `Relation` struct.
**Saved:** ~3 traits, removed unnecessary imports, simplified API.

## [Reduction]
**Bloat:** `relvar-core/src/types/user_defined_test.rs` - a single-purpose test file in the source tree with non-standard naming and location.
**Cut:** Moved tests to `relvar-core/src/values/scalar.rs` (where `ScalarValue` is defined) and deleted the file.
**Saved:** 1 file, removed confusing module structure, consolidated tests with the code they test.
