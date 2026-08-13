# ⚒️ Forge: Refactor God Function compute_pairwise_forces in physics

🚰 Smell: `relvar/src/experimental/physics.rs` contained a "God Function" `compute_pairwise_forces` (81 lines) which combined preparing the pairwise interactions via cross join and restriction, and then extending them with calculated forces. This violated the 'Three-Phase Operator' pattern.

✨ Solution: Extracted the logic into private helper functions `prepare_particle_pairs` and `compute_forces_for_interactions`.

🧼 Benefit: This flattens the execution flow and drastically increases code readability without changing logic.

🛡️ Verification: Tests passed. No logic changed.
