⚒️ Forge: Refactor Spreadsheet Evaluation God Function

🚮 Smell: The `evaluate` method inside `relvar/src/experimental/spreadsheet.rs` was an overly long "God Function" (66 lines) that combined filtering unresolved formulas, joining arguments, mathematically evaluating the resolved results, and managing the fixpoint loop in a single block.
✨ Solution: Applied the 'Three-Phase Operator' pattern by extracting the inner workings of the loop into three cleanly typed private helper functions: `get_unresolved_formulas`, `resolve_arguments`, and `evaluate_formulas`.
🧼 Benefit: Flattens the main evaluation loop, reducing cognitive load and dramatically improving code readability by abstracting the relational steps into descriptively named helpers.
🛡️ Verification: Tests passed (`cargo test --package relvar spreadsheet`). No logic changed.
