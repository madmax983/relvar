🎯 Target: `relvar/src/lib.rs` (`in_memory()`, `open()`) and `relvar-core/src/database/schema.rs` (`drop_virtual_relvar`, `drop_relvar`)
💣 Risk: High-level database orchestrator methods and facades were entirely untested in the core unit coverage suites, representing an undetected integration gap.
🧪 Strategy: Added direct module integration tests covering standard and simulated-engine database instantiation paths as well as view teardown code paths.
🔬 Verification: `cargo test -p relvar-core --test sentry_database_schema_coverage` and `cargo test -p relvar --lib`
