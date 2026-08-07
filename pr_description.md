Title: ⚒️ Forge: Extract God Function in Ungroup

🚮 Smell: The `compute_ungrouped_tuples` function in `relvar-core/src/algebra/group.rs` was a 55-line God Function combining capacity computation, relation extraction, base value extraction, and tuple building logic in a nested loop.
✨ Solution: Applied the "Three-Phase Operator" pattern by extracting `compute_total_capacity`, `extract_rva_relation`, `extract_base_values`, and `build_ungrouped_tuples` into private helper functions.
🧼 Benefit: Flattens the logic in the main loop, making the ungroup operation much easier to read and understand at a high level.
🛡️ Verification: Tests passed. No logic changed.
