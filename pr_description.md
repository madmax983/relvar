# ⚒️ Forge: Refactor physics compute_pairwise_forces

**🚮 Smell:**
The `compute_pairwise_forces` method in `relvar/src/experimental/physics.rs` was an 81-line "God Function". It combined renaming and joining particles to generate pairs, filtering self-interactions, and a massive `extend` closure to compute the `fx` and `fy` force components. This convoluted the relational logic with mathematical calculations, making it difficult to read and maintain.

**✨ Solution:**
Applied the "Three-Phase Operator" pattern by extracting the logic into two separate, private helper functions:
1. `generate_particle_pairs` which handles the cross joining, renaming, and filtering.
2. `compute_force_components` which takes the joined relations and computes the `fx` and `fy` components using `extend`.

**🧼 Benefit:**
Drastically improves clarity and reduces cognitive load by separating the relational setup from the physics calculations. The main function is now a clean, two-step pipeline. The new structure adheres strictly to idiomatic Rust standards for code organization and function length limits.

**🛡️ Verification:**
Tests passed successfully (`cargo test --all-features`). No logic was changed, ensuring this is strictly a refactor. All code styling (`cargo fmt`) and linting (`cargo clippy`) checks also passed with no warnings.