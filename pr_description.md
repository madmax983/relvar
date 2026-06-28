⚒️ Forge: Refactor Spreadsheet evaluation loop

🚮 Smell: The `evaluate` loop in `relvar/src/experimental/spreadsheet.rs` was a 66-line "God Function" that mixed finding unresolved formulas, resolving arguments, and calculating results inline.
✨ Solution: Applied the Three-Phase Operator pattern to extract `get_unresolved_formulas`, `resolve_formula_arguments`, and `compute_formula_results` as private helper functions.
🧼 Benefit: Dramatically flattens the evaluation loop, making the sequence of relational algebra operations easier to read and maintain.
🛡️ Verification: Tests passed. No logic changed.
