Title: ⚒️ Forge: Refactor ungroup and test setup functions

🚮 Smell: `compute_ungrouped_tuples` and `setup_db` were over 50 lines long, behaving as "God Functions" containing nested iterations and discrete distinct logic.
✨ Solution: Extracted inner logic into well-named private helper functions (`calculate_total_capacity`, `extract_base_values`, `process_rva_relation` in `group.rs`, and `setup_users_table`, `setup_orders_table` in `tests/common.rs`).
🧼 Benefit: Flattens the logic, separating intent from implementation, making the algorithms much easier to follow without deep nesting.
🛡️ Verification: Tests passed. No logic changed.
