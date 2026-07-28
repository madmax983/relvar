Title: ⚒️ Forge: Refactor `scan_visible` and `recover` functions

🚮 Smell: The `scan_visible` and `recover` functions were "God Functions" (61 lines each) that mixed multiple levels of abstraction, including scanning pages/WAL, evaluating conditions/visibility, and extracting/structuring data.
✨ Solution: Applied the "Three-Phase Operator" pattern. Extracted `identify_active_and_max_txns` and `collect_uncommitted_inserts` for `recover`. Extracted `scan_visible_page` and `extract_visible_tuple` for `scan_visible`.
🧼 Benefit: Flattens deeply nested structures, drastically improves readability, and separates concerns without changing behavior.
🛡️ Verification: Tests passed. No logic changed.
