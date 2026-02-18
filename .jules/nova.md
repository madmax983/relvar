# Nova's Journal - The Idea Graveyard

## [Initial Setup]
**Concept:** Setting up the journal.
**Fate:** Active
**Lesson:** Always start with a blank page.

## [Schema Visualizer]
**Concept:** A tool (`relvar::visualizer`) to generate Graphviz DOT files from the database schema, including Foreign Keys.
**Fate:** Merged
**Lesson:** Visualizations help users verify their mental model of the schema. Implemented as a pure function over `Database` without I/O.

## [Mock Data Generator]
**Concept:** A builder (`relvar::experimental::mock::MockRelation`) to generate random `Relation` data from a `RelationType` for testing and prototyping.
**Fate:** Merged
**Lesson:** Testing databases requires data. Generating it programmatically is faster than manual inserts. Used `rand` (isolated in experimental).

## [Pivot Operator]
**Concept:** A `pivot(on_attr, value_attr, default)` operator that transforms row values into column headers.
**Fate:** Merged
**Lesson:** Relational strictness (no NULLs) makes pivot tricky. Solution: Require a default value for sparse data.

## [The Importer]
**Concept:** A module (`relvar::experimental::importer`) to import Relations from JSON and CSV, strictly conforming to a `RelationType`.
**Fate:** Merged
**Lesson:** Symmetry is beautiful. If we have `exporter`, we need `importer`. Strict typing requires parsing intermediate JSON/String values against the schema, not just relying on `serde` defaults.

## [Spatial Types]
**Concept:** A module (`relvar::experimental::spatial`) implementing a `Point` type using `ScalarType::UserDefined` backed by `ScalarType::Relation`.
**Fate:** Proposed
**Lesson:** The relational model is powerful enough to represent complex types like Points without opaque blobs. By treating a Point as a relation `{x: Float, y: Float}`, we maintain purity and allow future extensibility while providing a familiar `Point` interface.

## [Relational Search]
**Concept:** A module (`relvar::experimental::search`) implementing Full-Text Search using an inverted index stored as a Relation.
**Fate:** Merged
**Lesson:** Search is just a join. By treating the inverted index as a relation `(term, doc_id, count)`, we can express search queries using standard relational algebra (Restrict -> Summarize), eliminating the need for a separate search engine for basic use cases.
