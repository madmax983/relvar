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
**Concept:** Modeled a Turing Machine's tape, head, and transitions purely as relations. Implemented the stepping logic using relational algebra operators (Join, Difference, Extend, Union, Project, Rename).
**Fate:** Merged
**Lesson:** Relational algebra is expressive enough to compute Turing Machine steps declaratively. Handling missing records (like a blank tape cell when the head moves to an uninitialized position) elegantly via set differences and conditional union makes it very powerful, proving that the engine is Turing complete.
