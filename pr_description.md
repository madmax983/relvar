Title: ⚒️ Forge: Extract God Function in Spreadsheet Evaluate

🚮 Smell: The `evaluate` method in `relvar/src/experimental/spreadsheet.rs` was an overly long "God Function" (66 lines) that combined finding unresolved formulas, joining with arguments, and evaluating expressions in a single loop body. This made the relational algebra steps hard to follow.
✨ Solution: Applied the "Three-Phase Operator" pattern by extracting `find_unresolved_formulas`, `resolve_arguments`, and `evaluate_formulas` into private helper functions. This flattens the main loop into readable iterations and clarifies the boundaries of the relational operations.
🧼 Benefit: Reduces cognitive load, significantly improving code readability and making the spreadsheet's relational algebra execution logic easier to maintain.
🛡️ Verification: Tests passed. No logic changed.