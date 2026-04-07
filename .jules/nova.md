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

## Relational Blockchain
**Concept:** A simplified blockchain simulation where blocks, transactions, and the ledger are represented as relations, and validation (e.g., balance checks, hash links) is done via relational algebra.
**Fate:** TBD
**Lesson:** TBD

## Relational Audio Synthesizer
**Concept:** Designed an audio synthesizer where sound generation is computed by applying relational algebra (Join, Extend, Summarize) to a timeline relation and an oscillators relation to compute mixed audio samples.
**Fate:** Merged
**Lesson:** Time-series and mathematical audio wave synthesis can be completely expressed using declarative relational structures and functional extensions, turning digital signal processing into a database querying task.

## Relational N-Body Physics Engine
**Concept:** Modeled an N-body gravity simulation where particles are relations, and interactions (pairwise gravity) and kinematics are computed using pure relational algebra operators (Join, Extend, Summarize, Restrict).
**Fate:** TBD
**Lesson:** TBD

## Relational Version Control System (RelGit)
**Concept:** Implemented a Git-like VCS where blobs, trees, commits, and branches are pure relations. Operations like checkout and computing diffs are fully expressed using relational algebra (Join, Difference, Union, Extend, Restrict).
**Fate:** Merged
**Lesson:** Version control is fundamentally a relational problem! Calculating diffs gracefully maps to set differences and intersections of joined trees and blobs, elegantly demonstrating how trees and file histories map to relational schemas without procedural traversal algorithms.

## Relational Neural Networks
**Concept:** Modeled a basic feedforward neural network using purely relational algebra. Layers, activations, weights, and biases are all relations. Forward propagation is evaluated using Join, Extend, and Summarize.
**Fate:** Merged
**Lesson:** Relational algebra maps exceptionally well to matrix operations and graph traversals. By chaining joins and extends, we can naturally express dense neural network connections without loops or procedural code!

## Relational Logic Circuit Simulator
**Concept:** A synchronous digital logic circuit evaluated using pure relational algebra. Gates and wires are represented as relations, and each step evaluates one propagation delay via joins and logic gate extensions.
**Fate:** Merged
**Lesson:** Logic gates act exactly like database queries filtering and extending values over wires, beautifully showing that hardware simulations can be modeled entirely relationally!
