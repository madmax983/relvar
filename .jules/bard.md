## 2025-04-09 - The "Missing" Examples
**Confusion:** A significant number of public functions and structs in the codebase lack executable doc-tests (`/// # Examples` blocks). This makes it difficult for users to understand how to use these APIs, particularly in experimental modules like `graph`, `ecs`, `blockchain`, `matrix`, `neural_network`, `raytracer`, `synth`, `turing`, etc.
**Clarification:** Executable doc-tests have been systematically added to public structs and methods to ensure every module tells a story and provides a copy-pasteable usage example, fulfilling Bard's philosophy that "A good example is worth 1,000 lines of explanation."
## 2025-04-10 - Undocumented Core Constraint Error Types
**Confusion:** Many enum error types and structures in constraints (e.g. `CheckConstraintError`, `TypeConstraintError`) were missing code examples.
**Clarification:** I added `/// # Example` with compileable tests for error types and structures across `check`, `manager`, `key`, `foreign_key`, `type_constraint`, and `expression` in `relvar-core/src/constraints` to guide users in dealing with validation errors.
