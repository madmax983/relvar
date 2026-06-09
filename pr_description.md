⚒️ Forge: Refactor Spreadsheet Evaluate Function

🚮 Smell: The `evaluate` method in `relvar/src/experimental/spreadsheet.rs` was a "God Function". It combined finding unresolved formulas, preparing mathematical arguments, evaluating operations, and handling result union all inside a single large loop block, hurting readability.

✨ Solution: Extracted the logic into three descriptive helper functions: `find_unresolved_formulas`, `prepare_arguments`, and `evaluate_formulas`.

🧼 Benefit: Dramatically improves readability by applying the Three-Phase Operator pattern. The main loop is now very short, flat, and its phases are clearly defined.

🛡️ Verification: `cargo test` passes. Behavior is unchanged as this was purely an extraction refactoring.
