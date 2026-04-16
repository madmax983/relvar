## 2025-04-09 - The "Missing" Examples
**Confusion:** A significant number of public functions and structs in the codebase lack executable doc-tests (`/// # Examples` blocks). This makes it difficult for users to understand how to use these APIs, particularly in experimental modules like `graph`, `ecs`, `blockchain`, `matrix`, `neural_network`, `raytracer`, `synth`, `turing`, etc.
**Clarification:** Executable doc-tests have been systematically added to public structs and methods to ensure every module tells a story and provides a copy-pasteable usage example, fulfilling Bard's philosophy that "A good example is worth 1,000 lines of explanation."
## 2024-05-15 - [Documenting RelationalVM]
**Confusion:** The experimental `RelationalVM` was poorly documented with a placeholder example and missing doc-tests for `new`, `step`, and `run` methods. It was unclear how it uses relational algebra to model VM state transitions.
**Clarification:** Added executable `/// # Examples` doc-tests for the struct and its methods, explicitly documenting how they initialize the VM and apply relational operators (join, restrict, extend, etc.) to perform state updates.
