## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.

## Relational Cellular Automata (Conway's Game of Life)
**Concept:** Modeled Conway's Game of Life using pure relational algebra. The grid is a relation `{x: Int, y: Int}` of live cells. Generations are computed via Cartesian product with neighbor deltas, extending for absolute coordinates, summarizing to count neighbors, and then using Natural Join and Restrict to find surviving cells and births.
**Fate:** Merged
**Lesson:** Relational algebra easily maps grid-based automata rules into declarative sets by using Cartesian products for neighbors and aggregations for rule evaluation, removing all loop-based traversal.
