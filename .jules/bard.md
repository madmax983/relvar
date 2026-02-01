# Bard's Journal

## The Case of the Vanishing Attributes
**Confusion:** The `theta_join` operation silently drops attributes from the right-hand relation if they share a name with an attribute in the left-hand relation.
**Clarification:** This is a known limitation of the current implementation (lack of automatic renaming). Documented this behavior explicitly in `relvar-core/src/algebra/join.rs` and added a regression test to ensure it remains consistent until a proper renaming mechanism is implemented.

## The Case of the Hidden Traits
**Confusion:** Users could not use advanced operators like `extend`, `summarize`, and `group` because the necessary traits were not re-exported, requiring deep and undocumented import paths (e.g. `relvar::algebra::extend::ExtendOps`).
**Clarification:** Re-exported `ExtendOps`, `GroupOps`, and `SummarizeOps` in `relvar-core/src/algebra/mod.rs` (accessible via `relvar::algebra::*`) and added comprehensive examples to the main `relvar` crate documentation demonstrating their usage and the required imports.

## The Case of the Theoretical Division
**Confusion:** The `divide` operator's documentation relied on a text-based ASCII art example that described the mathematical concept but didn't show how to actually use the Rust API to perform division.
**Clarification:** Replaced the text block with a fully executable `doctest` that constructs the classic "Suppliers and Parts" example relations and verifies the division result programmatically.

## The Case of the Undocumented Constraints
**Confusion:** The critical database constraint methods (`set_key_constraints`, `set_foreign_key_constraints`, `set_type_constraints`) had no examples, leaving users to guess how to construct the complex constraint objects.
**Clarification:** Added comprehensive `## Example` sections to each method, demonstrating the full workflow of creating relation types, instantiating constraint objects, and applying them to the database.

## The Case of the Missing Operator
**Confusion:** The `divide` operator was implemented and re-exported, but missing from the "Operators" summary table in the `relvar-core/src/algebra/mod.rs` documentation, making it undiscoverable via the module index.
**Clarification:** Added `Division` to the "Join Operators" table in the module-level documentation with a link to the `divide()` method.
