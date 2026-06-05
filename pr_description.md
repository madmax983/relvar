Title: ⚒️ Forge: Refactor compute_pairwise_forces

🚮 Smell: `compute_pairwise_forces` is an 81-line God Function that mixes renaming, joining, restricting, and extending logic.
✨ Solution: Extracted the logic into `cross_join_particles`, `restrict_self_interactions`, and `compute_force_components`.
🧼 Benefit: Reduces cognitive load and adheres to the Three-Phase Operator pattern for relational algebra.
🛡️ Verification: Tests passed. No logic changed.
