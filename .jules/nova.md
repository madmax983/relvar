## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.

## Relational Automata (NFA/DFA)
**Concept:** Implemented `RelationalNFA` that uses pure relational algebra (`join`, `project`, `rename`, `intersect`) to simulate finite state machines. The current states, transition table, and inputs are all Relations, naturally modeling non-determinism via sets.
**Fate:** Merged
**Lesson:** Relational algebra is extremely well-suited for modeling state machines, especially NFAs, because its set-based semantics elegantly handle multiple possible active states simultaneously without complex branching logic.
