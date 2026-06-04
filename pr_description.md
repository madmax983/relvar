Title: ⚒️ Forge: Refactor Spreadsheet God Function

🚮 Smell: The `evaluate` method in `Spreadsheet` (`relvar/src/experimental/spreadsheet.rs`) was a massive "God Function" (66 lines). It mixed multiple distinct phases of relational algebra evaluation into a single unstructured loop, harming readability.

✨ Solution: Refactored `evaluate` using the "Three-Phase Operator" pattern. Extracted `get_unresolved_formulas`, `resolve_formula_arguments`, and `evaluate_resolved_formulas` into strictly-typed private helper methods.

🧼 Benefit: Drastically flattens the execution flow of the main evaluation loop. Each relational algebraic step is now explicitly named, scoped, and separated into discrete operations, improving both documentation and maintainability without changing runtime behavior.

🛡️ Verification: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all` completed successfully. No logic was altered.