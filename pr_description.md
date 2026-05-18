⚒️ Forge: Refactored God Functions in GNN and Physics modules

🚮 Smell: Functions like `forward` in GNN and `compute_pairwise_forces` in Physics were acting as God Functions, mixing distinct relational operations (message propagation/linear transformation and pair generation/force calculation) into massive blocks of code, making them difficult to read and understand.
✨ Solution: Applied the 'Three-Phase Operator' pattern to extract these phases into cleanly typed private helper functions (`propagate_messages`, `apply_linear_transformation`, `generate_interacting_pairs`, `calculate_forces`), flattening the main methods.
🧼 Benefit: Significantly improves readability, clarity, and adherence to DRY principles. By explicitly scoping the relational operations, future modifications can be done with greater confidence and reduced cognitive load.
🛡️ Verification: Tests passed. No logic changed.
