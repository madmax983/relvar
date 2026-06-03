Title: 🛡️ Sentry: [test coverage improvement]

🎯 Target: `Relation::restrict_into` in `relvar-core/src/values/relation.rs` and `Relation::restrict` in `relvar-core/src/algebra/restrict.rs`.
💣 Risk: `restrict_into` lacked any test coverage, leaving its in-place mutation logic via `retain` unverified. `restrict` lacked coverage for stateful closures.
🧪 Strategy: Added tests for `restrict_into` to verify filtering, empty relations, and stateful closure usage. Added a test for `restrict` to ensure stateful closures behave correctly.
🔭 Verification: `cargo test` runs all these tests and ensures the core operations are stable.
