Title: ⚒️ Forge: [Refactor compute_ungrouped_tuples God Function]

🚮 Smell: The `compute_ungrouped_tuples` function in `relvar-core/src/algebra/group.rs` was a "God Function" (over 50 lines) that mixed calculating the exact needed capacity for pre-allocation with the actual logic to generate the ungrouped tuples. This obfuscated the relational operation.
✨ Solution: Extracted the logical phases into private helper functions: `calculate_ungrouped_capacity` and `generate_ungrouped_tuples`.
🧼 Benefit: Improves readability and flattens the main method without sacrificing efficiency or changing behavior.
🛡️ Verification: Tests passed. No logic changed.
