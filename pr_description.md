🛡️ Sentry: [test coverage improvement]

🎯 Target: `database/dml.rs`, `database/integrity.rs`, `types/scalar.rs`
💣 Risk: Missing coverage for updating tuples with mismatched types, constraint setters/getters, and user-defined scalar type comparisons could lead to uncaught logic bugs.
🧪 Strategy: Added specific unit tests to exercise these uncovered paths.
🔬 Verification: `cargo test`
