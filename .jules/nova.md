## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.
## Relational Matrix Math
**Concept:** Implemented `Matrix` struct and algorithms (addition, multiplication, transpose) modeling sparse matrices as relations with `row`, `col`, and `val` attributes, computing via pure relational algebra (Join, Extend, Summarize, Union).
**Fate:** Merged
**Lesson:** Representing matrices as coordinate lists via relations is extremely concise and completely abstracts away looping. Linear algebra maps quite naturally to Relational Algebra primitives.
