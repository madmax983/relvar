## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.
## Relational Finite State Automaton
**Concept:** Implemented `FiniteStateAutomaton` evaluation using purely relational algebra (Join, Project, Rename, Intersect).
**Fate:** Merged
**Lesson:** Relational algebra is highly effective for simulating state machines, as the concept of "active states" naturally expands into a relation. Joining with a transition relation seamlessly evaluates all active paths concurrently, essentially solving NFAs intrinsically without explicit backtracking.
