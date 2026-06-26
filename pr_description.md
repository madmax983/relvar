⚒️ Forge: Extract God Function in Spreadsheet evaluate

🚮 Smell: The `evaluate` method in `relvar/src/experimental/spreadsheet.rs` was a God Function combining antijoins for finding unresolved formulas, joining for argument resolution, and mapping logic for expression evaluation, all inside a single loop.
✨ Solution: Applied the 'Three-Phase Operator' pattern to extract `get_unresolved_formulas`, `resolve_arguments`, and `compute_results` into private helper functions.
🧼 Benefit: Drastically flattens the loop body, reducing cognitive load and clarifying the distinct relational phases.
🛡️ Verification: Tests passed. No logic changed.
