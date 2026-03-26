## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.

## Relational Raytracer
**Concept:** A basic 3D raytracer that models a scene and renders pixels using purely relational algebra (Cross Join, Extend, Restrict, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is expressive enough to compute geometric intersections and resolve Z-buffers declaratively, pushing the boundaries of what is considered "data" vs "compute."

## Relational Turing Machine
**Concept:** Modeled `tape`, `head`, and `transitions` as relations and computed steps purely through relational algebra (Join, Extend, Difference, Union) to demonstrate that the engine is Turing Complete.
**Fate:** Merged
**Lesson:** Relational algebra is expressive enough to compute arbitrary algorithms, acting as a functional paradigm for data. Simulating a Turing Machine step by step involves applying transitions via joins and updating the tape with sets.
