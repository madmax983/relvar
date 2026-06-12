⚒️ Forge: Refactor God Function in Physics Engine

🚮 Smell: The `compute_pairwise_forces` function in `relvar/src/experimental/physics.rs` was a "God Function" (81 lines) that mixed renaming relations, cross joining, filtering self-interactions, and complex closure execution for force calculation.
✨ Solution: Applied the "Three-Phase Operator" pattern by extracting `prepare_interactions` and `calculate_forces` into separate helper functions.
🧼 Benefit: Significantly improves readability by flattening the logic and clarifying the boundaries between relational algebra steps.
🛡️ Verification: Tests passed (`cargo test --package relvar physics`). No logic changed.
