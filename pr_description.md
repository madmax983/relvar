# ⚒️ Forge: [refactor image processing]

## 🚮 Smell
The `compute_kernel_contributions` function in `relvar/src/experimental/image.rs` was a classic "God Function" (74 lines long). Inside a loop, it mixed three distinct relational operations: shifting coordinates (`extend`), weighting color components (`extend`), and adding kernel indices before projecting to a standard schema. This large block of nested relational operations made the execution flow hard to read and violated the "Three-Phase Operator" pattern.

## ✨ Solution
Applied the "Three-Phase Operator" pattern to flatten the structure. The core loop body was extracted into three strictly-typed, descriptive private helper functions:
1. `extend_target_coordinates`: Handles shifting the coordinates (`tx` and `ty`).
2. `extend_weighted_colors`: Computes the weighted `r`, `g`, `b` components.
3. `extend_kernel_idx_and_project`: Appends the kernel index and finalizes the schema mapping.

## 🧼 Benefit
Dramatically improves readability and decreases cognitive load. The main loop in `compute_kernel_contributions` is now simply a pipeline of three clearly named steps. Types and boundaries act as documentation, and nesting is reduced, strictly following the Forge philosophy of flattened, understandable code.

## 🛡️ Verification
Ran `cargo test` and `cargo clippy`. Tests passed. No logic was changed, and runtime behavior remains exactly identical.
