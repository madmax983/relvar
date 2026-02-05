## [Reduction]
**Bloat:** Single-implementation extension traits (`ExtendOps`, `GroupOps`, `SummarizeOps`) for `Relation`.
**Cut:** Moved methods directly to `Relation` struct.
**Saved:** ~3 traits, removed unnecessary imports, simplified API.

## [Reduction]
**Bloat:** `relvar-core/src/types/user_defined_test.rs` - a single-purpose test file in the source tree with non-standard naming and location.
**Cut:** Moved tests to `relvar-core/src/values/scalar.rs` (where `ScalarValue` is defined) and deleted the file.
**Saved:** 1 file, removed confusing module structure, consolidated tests with the code they test.

## [Reduction]
**Bloat:** `relvar-storage/src/storage/btree.rs` - unused, speculative `BTreeIndex` implementation (placeholder wrapper around `BTreeMap`).
**Cut:** Deleted the file and removed exports.
**Saved:** ~400 lines of dead code + cognitive load (removed misleading architecture diagram components).

## [Reduction]
**Bloat:** `relvar-storage/src/storage/type_serializer.rs` and `system_relvars.rs` - unused custom serialization and unimplemented system catalog definitions ("Future Proofing" burden).
**Cut:** Deleted both files and removed exports.
**Saved:** ~400 lines of dead code, removed unnecessary maintenance burden for unimplemented features.

## [Reduction]
**Bloat:** `relvar/src/experimental/exporter.rs` - `Exporter` struct was a "Factory Factory" / unnecessary wrapper around `Relation` just to call methods.
**Cut:** Refactored into free functions (`to_csv`, `to_json`, `to_ascii_table`).
**Saved:** Simplified API (no need to instantiate `Exporter`), reduced boilerplate.
