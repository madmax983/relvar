Title: 🛡️ Sentry: [test coverage improvement]

🎯 Target: `Database` struct and DML operations.
💣 Risk: Missing test coverage for error conditions like type mismatches in `update` and operations on virtual relvars, which might cause undetected panics.
🧪 Strategy: Added internal tests in `database::data`, `database::dml`, `database::integrity`, and `database::transaction` modules.
🔬 Verification: `cargo test`
