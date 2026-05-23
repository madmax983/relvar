🚮 Smell: The codebase contained a "God Function" (`setup_db` in `relvar-core/src/query/tests/common.rs`) over 50 lines long. It conflated multi-step relational processing into monolithic blocks, severely hurting readability, creating pyramids of doom, and violating the Single Responsibility Principle.

✨ Solution: Applied the 'Three-Phase Operator' pattern to this God Function. Extracted logic into focused, strictly typed private helper functions `setup_users` and `setup_orders`.

🧼 Benefit: Dramatically improved code clarity by reducing nesting, flattening execution flow, and enforcing clear domain boundaries for logical phases without altering runtime behavior.

🛡️ Verification: Tests passed. No logic changed.
