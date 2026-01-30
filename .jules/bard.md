# Bard's Journal

## 2024-05-23 - The Case of the Vanishing Attributes
**Confusion:** The `theta_join` operation silently drops attributes from the right-hand relation if they share a name with an attribute in the left-hand relation.
**Clarification:** This is a known limitation of the current implementation (lack of automatic renaming). Documented this behavior explicitly in `relvar-core/src/algebra/join.rs` and added a regression test to ensure it remains consistent until a proper renaming mechanism is implemented.

## 2024-05-24 - The Case of the Hidden Traits
**Confusion:** Users could not use advanced operators like `extend`, `summarize`, and `group` because the necessary traits were not re-exported, requiring deep and undocumented import paths (e.g. `relvar::algebra::extend::ExtendOps`).
**Clarification:** Re-exported `ExtendOps`, `GroupOps`, and `SummarizeOps` in `relvar-core/src/algebra/mod.rs` (accessible via `relvar::algebra::*`) and added comprehensive examples to the main `relvar` crate documentation demonstrating their usage and the required imports.
