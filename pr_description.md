Title: ⚒️ Forge: Extract tuple generation in ungroup

🚮 Smell: The `compute_ungrouped_tuples` function in `group.rs` was a God Function (55 lines), combining validation, schema extraction, and the inner Cartesian product loop for tuple generation.
✨ Solution: Extracted the inner loop into a separate helper function `generate_ungrouped_tuples`.
🧼 Benefit: Flattens the deep nesting and isolates the Cartesian product logic, improving readability. Also replaced iterating over `tuple.values().iter()` to just `.keys()` for clippy.
🛡️ Verification: Tests passed. No logic changed.
