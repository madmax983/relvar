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
**Concept:** Modeled a Turing Machine purely with relations for tape, head position, and transition rules. Evaluated machine steps with pure relational algebra (Join, Extend, Difference, Union).
**Fate:** Merged
**Lesson:** Even Turing completeness can be modeled via relational operations, proving that the algebra is incredibly expressive for arbitrary iterative state transitions over unbounded tapes.

## Relational Audio Synthesizer
**Concept:** Designed an audio synthesizer where sound generation is computed by applying relational algebra (Join, Extend, Summarize) to a timeline relation and an oscillators relation to compute mixed audio samples.
**Fate:** Merged
**Lesson:** Time-series and mathematical audio wave synthesis can be completely expressed using declarative relational structures and functional extensions, turning digital signal processing into a database querying task.
