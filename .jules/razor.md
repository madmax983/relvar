## [Reduction]
**Bloat:** Single-implementation extension traits (`ExtendOps`, `GroupOps`, `SummarizeOps`) for `Relation`.
**Cut:** Moved methods directly to `Relation` struct.
**Saved:** ~3 traits, removed unnecessary imports, simplified API.

## [Reduction]
**Bloat:** `ConstraintManager` struct in `relvar-core` (Delegate pattern/Layer Lasagna).
**Cut:** Flattened fields and methods directly into `Database` struct.
**Saved:** ~200 lines of delegation boilerplate, 1 struct, removed unnecessary `&mut Engine` passing.

## [Reduction]
**Bloat:** `Exporter` struct in `relvar::experimental` (Wrapper struct/Method Object).
**Cut:** Replaced with free functions (`to_csv`, `to_json`, etc.).
**Saved:** 1 struct, simplified API usage.

## [Reduction]
**Bloat:** `SchemaVisualizer` struct in `relvar::visualizer` (Wrapper struct).
**Cut:** Replaced with free function (`to_dot`).
**Saved:** 1 struct, simplified API usage.
