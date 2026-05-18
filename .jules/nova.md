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

## Relational Automata
**Concept:** Modeled Non-deterministic Finite Automata (NFA) using purely relational algebra. States and transitions are relations, and string matching is evaluated iteratively using joins, epsilon closure with tclose, and projects.
**Fate:** TBD
**Lesson:** TBD

## Relational Knowledge Graph (RDF/SPARQL)
**Concept:** Modeled a Knowledge Graph using purely relational algebra. Implemented `TriplePattern` matching and Basic Graph Pattern (BGP) evaluation using chained Natural Joins over a unified `triples` relation to resolve SPARQL-like variable bindings.
**Fate:** TBD
**Lesson:** TBD
## Relational Sudoku Solver
**Concept:** Modeled a Sudoku solver using purely relational algebra. Cells and domain are relations, constraints are evaluated with joins, and domain reduction and propagation are performed via relational difference, summarization and union.
**Fate:** Merged
**Lesson:** Constraint satisfaction problems (like Sudoku) are perfectly suited to set-based relational operations. Set difference iteratively narrows valid domains and aggregations find cells that are fully determined.

## Relational Enigma Machine
**Concept:** Modeled an Enigma Machine purely with relations for plugboard, rotors, and reflector. Evaluated message encryption in parallel by resolving the electrical pathways via relational joins and computing rotation offsets via extensions.
**Fate:** TBD
**Lesson:** TBD
## Expert System
**Concept:** I built a Relational Expert System (`ExpertSystem`) using forward chaining that executes over `Facts`, `Rule Conditions`, and `Rule Conclusions` relations. It iteratively joins facts against conditions, verifies rules have all criteria met, and extracts conclusions to insert as new facts until reaching a fixpoint.
**Fate:** Merged
**Lesson:** Relational algebra handles state machines and derivation well! We must use `Aggregation::count` coupled with `summarize` and `restrict` to ensure all conditions are fulfilled before pulling the conclusion from `rule_conclusions`. It perfectly captures the essence of rule engines in the Relational framework!
## Relational K-Means Clustering
**Concept:** Implemented `KMeans` clustering using purely relational algebra (Join, Extend, Summarize). Represented points and centroids as relations, performing nearest neighbor assignment and centroid updating via standard relation operations like aggregation and cartesian products.
**Fate:** Merged
**Lesson:** Relational algebra proves surprisingly versatile for vector-based machine learning algorithms. Cross joins correctly compute pairwise distances, and `Aggregation::min` with joins elegantly act as an `argmin` function to assign the closest points cleanly.
## Relational Virtual Machine
**Concept:** Modeled a basic Virtual Machine purely with relational algebra. Registers, Memory, and the Program are relations. Instruction evaluation and state progression (e.g., executing an ADD instruction) are performed via Relational Joins, Extensions, and Unions.
**Fate:** TBD
**Lesson:** TBD
## Relational Role-Based Access Control (RBAC)
**Concept:** Modeled a full RBAC system with role hierarchies using pure relational algebra. Uses transitive closure (`tclose`) on the role hierarchy to find all ancestor roles, `union` to combine direct roles with inherited roles, and `join` with permissions to resolve effective permissions.
**Fate:** Merged
**Lesson:** Relational algebra gracefully handles complex inheritance trees by combining the power of `tclose` (for evaluating recursion) and natural `join`s (for mapping user entitlements), providing a purely declarative approach to security resolving.
## Relational CYK Parser
**Concept:** Modeled a CYK parser for Context-Free Grammars in Chomsky Normal Form purely with relational algebra. Uses relations for `Terminals`, `Non-Terminals`, and `Input`. The parse table is built iteratively via relational Joins and Extensions until reaching a fixpoint.
**Fate:** TBD
**Lesson:** Relational algebra easily maps well to dynamic programming problems like the CYK parsing algorithm where results from overlapping sub-problems can be joined and extended dynamically!
## Relational Markov Chain
**Concept:** Modeled a Markov Chain using purely relational algebra (Join, Extend, Summarize). State distributions and transition matrices are purely relational.
**Fate:** Merged
**Lesson:** Stochastic processes can be elegantly mapped to relational algebra! By joining probability distributions with transition tables and extending with multiplied float probabilities, relational algebra acts as a pure matrix multiplication engine for vector distributions.
## Relational Genetic Algorithm
**Concept:** Modeled a Genetic Algorithm where the population is a relation. Fitness evaluation is computed using `Extend`, selection of the top N individuals is elegantly handled using a relational ranking pattern (via Theta-Join counting strictly better fitnesses, Summarize, and Restrict), and reproduction uses Join and Extend.
**Fate:** Merged
**Lesson:** Evolutionary algorithms map surprisingly well to declarative queries. By using a Theta-Join to compute a "rank", we can implement a pure relational Top-N filter without introducing any procedural loop sorting, allowing the entire algorithm step to be resolved natively as a database query evaluation.
## Relational Petri Net Simulator
**Concept:** Implemented a Petri Net simulator purely using relational algebra (`PetriNet`). `places`, `transitions`, `input_arcs`, and `output_arcs` are modeled as relations. Enabled transitions are discovered via chained relation joins and restrictions. Firing computes token deltas via relational unions and aggregation, ensuring completely declarative state evolution.
**Fate:** Merged
**Lesson:** Relational algebra is a perfect match for token-based state machines! By using set differences and aggregations, we can elegantly find satisfied network flows and update distributed state concurrently without procedural traversal algorithms.
## Cellular Automata
**Concept:** Modeled Conway's Game of Life purely using relational algebra (Cross Join, Extend, Summarize, Difference, Union). The infinite grid is represented sparsely as a relation of alive cells' coordinates.
**Fate:** Merged
**Lesson:** Relational algebra gracefully handles cellular automata by turning neighbor counting into cross joins and aggregations, and rule evaluation into set operations like restrictions and differences.

## Relational Graph Neural Network (GNN)
**Concept:** Modeled a Graph Neural Network (Message Passing) purely with relational algebra. Edges, Features, and Weights are represented as relations. The message passing algorithm is computed via Joining edges and features, Aggregating neighbor features (Sum), and Joining/Extending with Weights to compute linear transformations.
**Fate:** TBD
**Lesson:** TBD
## Relational Graph Neural Network (GNN)
**Concept:** Modeled a Graph Neural Network (Message Passing) purely with relational algebra. Edges, Features, and Weights are represented as relations. The message passing algorithm is computed via Joining edges and features, Aggregating neighbor features (Sum), and Joining/Extending with Weights to compute linear transformations.
**Fate:** Merged
**Lesson:** Relational joins act as sparse matrix multiplications. Message passing algorithms gracefully compile down into relational joins followed by aggregate functions, turning a database engine into a capable inference engine for graph data!
## Relational DOM Engine
**Concept:** Modeled a Document Object Model (DOM) where nodes, parent-child edges, and attributes are pure relations. CSS Selectors (like descendant selectors) are evaluated declaratively using transitive closure (`tclose`) on the edges relation, joining with the attributes relation.
**Fate:** TBD
**Lesson:** TBD

## Relational PageRank Algorithm
**Concept:** Modeled the PageRank algorithm using purely relational algebra. Evaluates PageRank iterations on an edge relation via chained Natural Joins, Renames, Extensions, and Aggregations to iteratively distribute rank score without a looping procedural graph traversal.
**Fate:** Merged
**Lesson:** Relational algebra maps exceptionally well to eigenvector centrality problems like PageRank! By expressing out-degree calculation as a summarization and rank distribution as a joined extension, graph analysis compiles neatly down into relational engine queries.
## Relational Build System
**Concept:** Modeled a Build System (like Make/Ninja) using pure relational algebra. Dependencies are an edges relation, and file timestamps are a relation. Stale targets are resolved by finding files older than their dependencies via transitive closure (`tclose`), relational joins, and aggregations.
**Fate:** TBD
**Lesson:** TBD
## Relational Garbage Collector
**Concept:** Modeled a Mark-and-Sweep Garbage Collector using purely relational algebra. Represents `roots`, `heap`, and `references` as relations. The Mark phase computes reachability via `tclose`, `join`, and `union`. The Sweep phase identifies garbage via `difference`.
**Fate:** TBD
**Lesson:** Relational algebra gracefully handles complex algorithms like Garbage Collection! By leveraging transitive closure to compute reachable sets and difference to identify unreferenced memory, we can declaratively specify Mark-and-Sweep.
## Relational Knowledge Graph (RDF/SPARQL)
**Concept:** Modeled a Knowledge Graph using purely relational algebra. Implemented `TriplePattern` matching and Basic Graph Pattern (BGP) evaluation using chained Natural Joins over a unified `triples` relation to resolve SPARQL-like variable bindings.
**Fate:** Merged
**Lesson:** Relational algebra maps exceptionally well to evaluating basic graph patterns. By projecting and renaming bound variables and running Natural Joins, the relational engine seamlessly resolves complex graph queries.
## Relational Spreadsheet Simulator\n**Concept:** Modeled a Spreadsheet Simulator purely using relational algebra. Cells and formulas are represented as relations. The spreadsheet is iteratively evaluated using relation differences, joins, and extensions until all cell formulas are fully resolved to their final float values.\n**Fate:** Merged\n**Lesson:** Relational engines can model constraint propagation like a spreadsheet! By continually computing the difference between resolved values and formula arguments, we can declaratively resolve dependencies without constructing explicit dependency graphs or recursion.
## Relational Fourier Transform
**Concept:** Modeled the Discrete Fourier Transform purely with relational algebra.
**Fate:** TBD
**Lesson:** TBD
