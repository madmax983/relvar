## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.
## Relational Cellular Automata
**Concept:** Implemented Conway's Game of Life entirely using relational algebra (Cross Join, Extend, Summarize, Restrict, Union) over a relation of active cells `(x, y)`.
**Fate:** Merged
**Lesson:** Complex emergent behavior and spatial simulations can be elegantly modeled in a pure relational way without imperative loops, leveraging sets and self-joins to evaluate rules.
