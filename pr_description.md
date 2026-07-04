⚒️ Forge: Refactor God Function compute_pairwise_forces in Physics Engine

🚮 Smell: The `compute_pairwise_forces` method in `relvar/src/experimental/physics.rs` was a classic God Function (over 80 lines). It mixed joining relations, filtering interactions, and executing deep calculations for force components into a single massive block. This violated the "Three-Phase Operator" pattern and made the relational operations difficult to trace.

✨ Solution: Applied the "Three-Phase Operator" pattern by extracting the logical steps into descriptive, focused helper functions:
- `generate_particle_pairs`: Prepares and joins the relations.
- `filter_self_interactions`: Restricts the joined pairs.
- `calculate_force_components`: Extends the filtered relations with calculation logic.

🧼 Benefit: Dramatically improves readability, flattening the execution flow. The main method is now just 4 lines of clear declarative logic.

🛡️ Verification: Tests passed. No logic changed.
