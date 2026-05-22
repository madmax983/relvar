🚬 Smell: The `compute_kernel_contributions` function in `relvar/src/experimental/image.rs` was a "God Function" combining target coordinates creation, weighted color components logic, kernel index additions, and attribute renaming/projection all inline within a main loop.

✨ Solution: Extracted the inline inline loop blocks into separate private helper functions: `calculate_target_coordinates`, `calculate_weighted_colors`, `add_kernel_index`, and `project_and_rename_contribution` applying the Three-Phase Operator pattern.

🧽 Benefit: Vastly improves code readability and separation of concerns by flattening the massive loop operations and isolating distinct transformation steps.

🛡️ Verification: Tests passed. No logic changed. All tests related to convolution logic continue to pass.