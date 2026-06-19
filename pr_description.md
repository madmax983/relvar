🛡️ Sentry: Database error and validation coverage

🎯 Target: `Database` module (`relvar-core/src/database`), specifically `data.rs`, `dml.rs`, `integrity.rs`, and `schema.rs`.
💣 Risk: Untested error boundaries and bulk validation paths in Database constraints during `update` and `delete` statements. Missing integrity/schema interface unit tests for `CheckConstraints`, `ForeignKeyConstraints` and list API boundaries.
🧪 Strategy: Introduced `sentry_database_integrity_coverage.rs` and `sentry_database_schema_coverage.rs` for explicitly testing getters, setters, constraints tracking, and error bubbling paths. Added explicit missing boundary tests for parent/child constraints inside `sentry_database_dml_coverage.rs`.
🔬 Verification: Run `cargo test -p relvar-core test_database_integrity_`, `cargo test -p relvar-core --test sentry_database_schema_coverage`, and `cargo test -p relvar-core --test sentry_database_dml_coverage`.
