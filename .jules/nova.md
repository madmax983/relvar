# Nova's Journal - The Idea Graveyard

## [Initial Setup]
**Concept:** Setting up the journal.
**Fate:** Active
**Lesson:** Always start with a blank page.

## [Schema Visualizer]
**Concept:** A tool (`relvar::visualizer`) to generate Graphviz DOT files from the database schema, including Foreign Keys.
**Fate:** Merged
**Lesson:** Visualizations help users verify their mental model of the schema. Implemented as a pure function over `Database` without I/O.

## [Audit Logging]
**Concept:** A wrapper (`AuditedDatabase`) around `Database` that automatically logs all mutations (INSERT, UPDATE, DELETE) to an internal system relvar `_AUDIT_LOG`.
**Fate:** Submitted
**Lesson:** Using the database to track itself is a powerful pattern ("dogfooding"). Wrapper pattern avoids modifying core logic.
