⚒️ Forge: Refactored evaluate God Function in Spreadsheet

🚮 Smell: The evaluate method in relvar/src/experimental/spreadsheet.rs was a classic "God Function" (66 lines) that conflated finding unresolved formulas, joining arguments, and evaluating operations into a single massive loop.
✨ Solution: Applied the "Three-Phase Operator" pattern, extracting the logic into find_unresolved_formulas and evaluate_formulas private helper functions.
🧼 Benefit: Significantly flattens the main evaluation loop, making the relational boundaries much clearer and improving readability without altering any logic.
🛡️ Verification: Tests passed. No logic changed.
