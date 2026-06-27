⚒️ Forge: [Refactor] Flatten nesting with Guard clauses in constraints

🚮 Smell: Deeply nested `if let Some` blocks in constraint validation functions (`ConstraintManager`) and `Tuple::new`, along with manual `for` loops to find missing attributes.
✨ Solution: Extracted early returns using `let Some(...) = ... else { return; }` to flatten the guard clauses. Replaced a manual search loop with an idiomatic `.find()` iterator pipeline.
🧼 Benefit: Dramatically reduces indentation and cognitive load. Clearer "fast path" failure and success logic.
🛡️ Verification: Tests passed. No logic changed.
