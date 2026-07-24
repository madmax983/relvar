⚒️ Forge: Refactor compute_ungrouped_tuples god function

🚮 **Smell:** The `compute_ungrouped_tuples` function in `relvar-core/src/algebra/group.rs` was 55 lines long, suffering from the "God Function" smell. It mixed calculating total capacity for pre-allocation, extracting the RVA from each tuple, computing base attributes, and creating the new tuples.

✨ **Solution:** Applied the "Three-Phase Operator" pattern to extract logic into three focused, private helper functions: `calculate_total_capacity`, `extract_rva_relation`, and `compute_base_values`.

🧼 **Benefit:** Drastically improves readability by flattening the main function into clear, sequential logical phases.

🛡️ **Verification:** Tests passed. No logic changed.
