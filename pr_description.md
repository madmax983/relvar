# 🛡️ Sentry: [test coverage improvement]

🎯 **Target:** `relvar-core/src/database/dml.rs` (`compute_relation_after_delete`, `compute_relation_after_update`).
💣 **Risk:** The pure DML manipulation methods lacked basic unit tests, which could hide bugs in tuple mismatch checking or early exit error paths when computing update constraints before applying them to the DB.
🧪 **Strategy:** Added table-driven testing in a new `mod tests` block covering matching/no-matching branch conditions and schema mismatch error branches.
🔬 **Verification:** `cargo test --manifest-path relvar-core/Cargo.toml --lib database::dml::tests`
