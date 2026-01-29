# Bard's Journal

## 2024-05-23 - The Case of the Vanishing Attributes
**Confusion:** The `theta_join` operation silently drops attributes from the right-hand relation if they share a name with an attribute in the left-hand relation.
**Clarification:** This is a known limitation of the current implementation (lack of automatic renaming). Documented this behavior explicitly in `relvar-core/src/algebra/join.rs` and added a regression test to ensure it remains consistent until a proper renaming mechanism is implemented.
