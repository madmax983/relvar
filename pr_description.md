Title: ⚒️ Forge: Refactor compute_pairwise_forces God Function

🚮 Smell: The `compute_pairwise_forces` function was over 80 lines long, mixing relational joining, filtering, and deeply nested closures for calculating gravitational forces in the x and y axes.
✨ Solution: Applied the Three-Phase Operator pattern by extracting `generate_particle_pairs`, `filter_self_interactions`, and `calculate_pairwise_forces`. The duplicated physics math in the x and y closures was also extracted into `calculate_force_component`.
🧼 Benefit: Reduces cognitive load by separating the relational logic from the physics math, strictly typed and flattened structure.
🛡️ Verification: Tests passed. No logic changed.
