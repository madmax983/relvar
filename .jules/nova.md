## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.
## Relational Conway's Game of Life
**Concept:** Implement Conway's Game of Life purely using relational operators. The grid is represented as a sparse relation `{x, y}`, and each generation is evaluated via Join (neighborhood deltas), Extend, Summarize (neighbor counts), Restrict (rules of life), and Union.
**Fate:** TBD
**Lesson:** Relational algebra easily handles set-based spatial logic.
