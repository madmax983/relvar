🚮 Smell: Several large functions (`evaluate` in `spreadsheet.rs`, `query_descendant` in `dom.rs`, `diff`/`checkout` in `vcs.rs`, `match_pattern` in `knowledge_graph.rs`) were excessively long and handled multiple logical phases at once.
✨ Solution: Extracted the logical sub-phases into descriptive helper functions (e.g., `find_unresolved_formulas`, `evaluate_term`, `resolve_working_directory`).
🧼 Benefit: Reduces cognitive load by flattening structure, improves clarity without changing behavior.
🛡️ Verification: Tests passed. No logic changed.
