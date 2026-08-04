Title: 🛡️ Sentry: [test coverage improvement]

🎯 Target: relvar-core/src/database/dml.rs
💣 Risk: DML primitive logic could fail on edge cases such as mismatched tuples during updates, non-matching predicates, or deletion logic.
🧪 Strategy: Added a comprehensive unit test suite inside `relvar-core/src/database/dml.rs` (`mod tests`) to cover `compute_relation_after_delete`, `compute_relation_after_update`, and mismatched tuple type updates to improve code coverage.
🔭 Verification: cargo test -p relvar-core --lib database::dml::tests
