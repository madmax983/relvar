# ⚒️ Forge: Extract God Function in Physics Simulation

🚮 Smell: The `compute_pairwise_forces` method in `relvar/src/experimental/physics.rs` was an 81-line "God Function" that handled joining, filtering, and performing two massive extensions for computing X and Y force components in one continuous block.
✨ Solution: Applied the "Three-Phase Operator" pattern by extracting `generate_particle_pairs`, `filter_self_interactions`, and `calculate_force_components` into separate private helper functions.
🧼 Benefit: Significantly flattens the main control flow, making the relational physics steps highly readable without altering any underlying behavior.
🛡️ Verification: Tests passed. No logic changed.
