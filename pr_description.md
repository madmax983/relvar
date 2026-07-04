🛡️ Sentry: [test coverage improvement]
🎯 Target: `relvar-core/src/database/data.rs`, `relvar-core/src/database/integrity.rs`, `relvar-core/src/database/schema.rs`, `relvar-core/src/database/dml.rs`.
💣 Risk: Missing coverage for error paths via the `?` macro and pure functions that were written as returning `Result` types despite never failing. This artificially deflates coverage reporting and could obscure untested failure logic.
🧪 Strategy:
1. Added specific edge-case tests in `sentry_database_data_coverage.rs`, `sentry_database_integrity_coverage.rs`, and `sentry_database_schema_coverage.rs` that purposefully violate constraints, thereby hitting the un-tested `Err` execution paths from various `?` macros in the database orchestrator.
2. Removed an un-needed `Result` return type on the pure function `compute_relation_after_delete` which never actually failed, thus eliminating a false positive missing `Err` path coverage check without needing excessive refactoring.
3. Formatted `try_fold` block gracefully in `dml.rs`.
🔬 Verification: `cargo test` and `cargo tarpaulin --ignore-tests`.
