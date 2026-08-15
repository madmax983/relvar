# 🛡️ Sentry: [DML operations error path tests]

🎯 Target: `relvar-core/src/database/dml.rs` (`compute_relation_after_update` and `compute_relation_after_delete`)
💣 Risk: DML primitives missing direct test coverage of inner pure functions decoupling them from the storage layer, specifically leaving `TupleMismatch` untested.
🧪 Strategy: Add `mod tests` directly into `dml.rs` testing pure functions.
🔬 Verification: `cargo test`
