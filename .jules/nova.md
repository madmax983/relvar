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
