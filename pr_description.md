🛡️ Sentry: [test coverage improvement]
🎯 Target: `relvar-core::database` and specifically `data.rs` and `integrity.rs` operations (`insert`, `update`, `delete`).

💣 Risk: Missing test coverage for error conditions like constraint violations, modifications to non-existent relvars, modifications to virtual relvars, or returning incorrect tuple matches. This could lead to a panic in production or regressions when constraint validation logic is altered.

🧪 Strategy: Added specific unit tests to verify:
- Attempting `insert`, `update`, `delete` on nonexistent and virtual relvars results in proper errors instead of panics.
- Updates causing `TupleMismatch`.
- Constraints returning errors correctly (Key violation, Type Constraint violation, Check Constraint violation, Foreign Key violation) directly through `insert`, `update`, and `delete`.
- Validation of constraint effects within a transaction cycle.

🔬 Verification: Run `cargo test -p relvar-core --lib database`
