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
