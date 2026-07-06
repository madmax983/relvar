🚮 Smell: The `compute_pairwise_forces` function in `relvar/src/experimental/physics.rs` was 81 lines long, mixing relational join filtering with physics calculations.
✨ Solution: Extracted the logical stages into `generate_particle_interactions` and `calculate_forces` private helper functions.
🧼 Benefit: Clearly separates the logical mapping from the actual mathematical evaluation, resulting in flatter and more semantic flow.
🛡️ Verification: Tests passed. No logic changed.
