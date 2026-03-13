## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.
## Relational Cellular Automaton (Conway's Game of Life)
**Concept:** Modeled a grid of alive cells as a relation with `x` and `y` coordinates. Implemented generation advancement using cartesian products with offsets to map neighbors, `Summarize` with `Aggregation::count` to count them, and set operations (`Join`, `Restrict`, `Difference`, `Union`) to evaluate Conway's survival and birth rules.
**Fate:** Merged
**Lesson:** Relational algebra handles geometric spaces elegantly by mapping neighbor deltas via disjoint cartesian joins, making seemingly procedural simulation rules fully declarative.
