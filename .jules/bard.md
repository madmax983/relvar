## 2025-04-09 - The "Missing" Examples
**Confusion:** A significant number of public functions and structs in the codebase lack executable doc-tests (`/// # Examples` blocks). This makes it difficult for users to understand how to use these APIs, particularly in experimental modules like `graph`, `ecs`, `blockchain`, `matrix`, `neural_network`, `raytracer`, `synth`, `turing`, etc.
**Clarification:** Executable doc-tests have been systematically added to public structs and methods to ensure every module tells a story and provides a copy-pasteable usage example, fulfilling Bard's philosophy that "A good example is worth 1,000 lines of explanation."
## 2024-05-15 - [Documenting RelationalVM]
**Confusion:** The experimental `RelationalVM` was poorly documented with a placeholder example and missing doc-tests for `new`, `step`, and `run` methods. It was unclear how it uses relational algebra to model VM state transitions.
**Clarification:** Added executable `/// # Examples` doc-tests for the struct and its methods, explicitly documenting how they initialize the VM and apply relational operators (join, restrict, extend, etc.) to perform state updates.
## 2026-04-16 - The "extend_into" Example\n**Confusion:** The `extend_into` method was missing an executable doc-test, making it unclear how it differed from `extend`.\n**Clarification:** Added a comprehensive executable doc-test for `extend_into` to demonstrate how it consumes the relation and takes ownership of its tuples.
## 2025-04-18 - Missing Error Examples
**Confusion:** The core error enums, such as `DatabaseError`, lacked executable doctests, making it unclear how to match against and properly format the various errors thrown by the relational engine.
**Clarification:** Added an executable `/// # Examples` block to `DatabaseError` to clearly demonstrate matching variants and formatting the errors to strings.
## 2025-05-16 - The "PetriNet" Examples
**Confusion:** The experimental `PetriNet` module had no executable doc-tests for the main struct or its methods (`new`, `add_place`, `add_transition`, `add_input_arc`, `add_output_arc`, `enabled_transitions`, and `fire`). This lack of examples made it confusing to understand how to initialize the Petri Net, configure places and transitions, or execute simulation steps (firing transitions).
**Clarification:** Added executable `/// # Examples` doc-tests for `PetriNet` and all its methods, demonstrating how to construct a simple Petri Net and how transitions consume/produce tokens dynamically.
## 2026-04-22 - The "GraphNeuralNetwork" Examples
**Confusion:** The experimental `GraphNeuralNetwork` had no executable doc-tests for the main struct or its `forward` method. This lack of examples made it confusing to understand how to initialize the features, edges, and weights relations and how to execute the forward pass.
**Clarification:** Added executable `/// # Examples` doc-tests for `GraphNeuralNetwork` and its `forward` method, demonstrating how to construct the necessary relations and perform a forward pass.
## 2025-04-23 - Documenting Graph Neural Network
**Confusion:** The experimental `GraphNeuralNetwork` had a placeholder example and missing doc-tests for the main struct and its methods, making it confusing to understand how to initialize the features, edges, and execute the message aggregation pass.
**Clarification:** Added executable `/// # Examples` doc-tests for `RelationalGNN` and its methods, demonstrating how to construct the necessary relations and perform a forward pass.
## 2025-04-24 - The "RelationalDom" Examples
**Confusion:** The experimental `RelationalDom` had a placeholder example for the struct and lacked executable doc-tests for its methods (`new`, `query_class`, and `query_descendant`), making it difficult to understand how to construct the nodes, edges, and attributes relations or perform CSS-like queries.
**Clarification:** Replaced the placeholder and added comprehensive executable `/// # Examples` doc-tests for `RelationalDom` and all its methods to clearly demonstrate how to model a DOM and evaluate descendant selectors declaratively.
## 2025-05-18 - Documenting Internal Tests
**Confusion:** Added doc-tests to internal test modules (`relvar-core/src/database/tests/common.rs`) causing test failures when external paths could not be resolved by `cargo test --doc` due to `cfg(test)` access restrictions. The `find_undoc.py` script flagged these missing docs incorrectly.
**Clarification:** Excluded internal test directories from `find_undoc.py` and reverted doctests in `common.rs` to ensure documentation is strictly for public-facing, user-accessible APIs, keeping doctests verifiable and avoiding failing `cargo test --doc`.
## 2025-04-30 - Missing Module Level Docs\n**Confusion:** Various modules in `relvar-core/src/experimental/` and `relvar/src/experimental/` as well as `relvar-storage/src/wal/` were missing top-level module documentation (`//!`), which made it harder to understand the abstract concept before diving into specific types or functions.\n**Clarification:** Added missing module-level documentation to `difference.rs`, `game_of_life.rs`, `pagerank.rs`, `petri_net.rs`, and `iter.rs`, and added dummy module docs for test files that require them for test suite tooling, to ensure the abstract story is told.
## 2025-05-19 - The "GarbageCollector" Examples
**Confusion:** The experimental `GarbageCollector` module lacked executable doc-tests (`/// # Examples` blocks) for its struct and methods (`new` and `mark_and_sweep`). This made it difficult for users to understand how to instantiate the roots, heap, and references relations and run the GC algorithm.
**Clarification:** Added comprehensive executable doc-tests to `GarbageCollector`, `new`, and `mark_and_sweep` that demonstrate how to construct the necessary relations and see which objects are identified as garbage.
## 2024-05-18 - [Error Documentation]
**Confusion:** Error enums were documented with simple `.to_string()` assertions, which was "Dead End" noise failing to explain how to recover.
**Clarification:** Rewrote doc-tests to include rich `/// # Recovery` sections mapped to each variant, explaining exactly what the error means in the Relational context and how to fix it, using story-driven lore as per Bard's philosophy.
## 2025-05-19 - Documenting Knowledge Graph
**Confusion:** The experimental `KnowledgeGraph` had no executable doc-tests (`/// # Examples` blocks) for its struct or its core methods (`new`, `insert`, `query`), and its `TriplePattern` struct was similarly undocumented. This made it difficult for users to understand how to initialize the graph, insert triples, and construct patterns for querying.
**Clarification:** Added executable doc-tests to `KnowledgeGraph` and its methods that demonstrate how to construct the graph, insert relation triples, and query them.
