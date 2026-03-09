## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.

## Relational Cellular Automaton (Game of Life)
**Concept:** Implemented Conway's Game of Life purely using Relational Algebra. Cells are represented as `(x, y)` coordinates, neighbors as `(dx, dy)` deltas, and state transitions are handled by relational operators like Joins, Extends, Summarizes, and Unions.
**Fate:** Merged
**Lesson:** Relational Algebra is Turing-complete and capable of executing complex iterative simulations without relying on imperative logic at the data level. The elegance of "Cartesian Products + Aggregations" naturally expresses state transitions over a grid.
