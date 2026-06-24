🛡️ Sentry: [test coverage improvement]

🎯 Target: `relvar-core/src/algebra/summarize.rs` error paths.
💣 Risk: Missing validation test coverage on aggregation error states including float overflow boundaries and empty sets.
🧪 Strategy: Added explicit tests to trigger float overflows during AVG and SUM aggregations and verify bounds handling on empty set constraints. Also improved database integrity checks for virtual relvars and FKs.
🔬 Verification: `cargo test`
