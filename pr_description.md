Title: ⚒️ Forge: Refactor compute_pairwise_forces and compute_kernel_contributions God Functions

Description:
🚮 Smell: The `compute_pairwise_forces` method in `relvar/src/experimental/physics.rs` and the `compute_kernel_contributions` method in `relvar/src/experimental/image.rs` were long, complex "God Functions". They contained deeply nested relational `extend` logic directly in their core iterative structures, increasing cognitive load and making the flow of relation construction hard to read.
✨ Solution: Applied the "Three-Phase Operator" pattern. In `physics.rs`, extracted the massive `extend` logic calculating vector forces into `calculate_force_components`. In `image.rs`, extracted the inner kernel-tap processing loop (shifting, scaling, renaming, projecting) into a focused `compute_single_contribution` helper function.
🧼 Benefit: Flattened the core algorithms into readable coordination blocks, moving detailed relational arithmetic into clearly named, tightly scoped, private helper functions. Dramatically improves readability without altering behavior.
🛡️ Verification: Tests passed. No logic changed.