# Nova's Journal - The Idea Graveyard

## [Initial Setup]
**Concept:** Setting up the journal.
**Fate:** Active
**Lesson:** Always start with a blank page.

## [Schema Visualizer]
**Concept:** A tool (`relvar::visualizer`) to generate Graphviz DOT files from the database schema, including Foreign Keys.
**Fate:** Merged
**Lesson:** Visualizations help users verify their mental model of the schema. Implemented as a pure function over `Database` without I/O.

## [Semantic Relation Differ]
**Concept:** A tool (`relvar::experimental::differ`) to calculate the semantic difference (Insert/Update/Delete) between two Relations, optionally respecting keys.
**Fate:** Proposed
**Lesson:** Set theory is cool, but sometimes you just want to know "what changed?".
