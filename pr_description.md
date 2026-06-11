⚒️ Forge: Refactor compute_pairwise_forces
🚮 Smell: `compute_pairwise_forces` was 81 lines long with duplicated math inline.
✨ Solution: Extracted `generate_particle_pairs` and `compute_force_component` helper functions.
🧼 Benefit: Reduces cognitive load, flattens closures, increases clarity.
🛡️ Verification: Tests passed. No logic changed.
