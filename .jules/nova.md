## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.
## Relational Cellular Automaton
**Concept:** Implemented Conway's Game of Life purely using relational algebra (Cartesian Product, Extend, Summarize, Restrict, Union).
**Fate:** Merged
**Lesson:** Relational algebra can elegantly express spatial constraints and transition rules for cellular automata without iterative loops, specifically by exploiting relational joins for neighborhood generation and summarization for neighbor counts.
