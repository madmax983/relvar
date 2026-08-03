Title: 🛡️ Sentry: [fix ci errors]

🎯 Target: Multiple modules across the codebase.
💣 Risk: CI failing due to private module accesses and clippy warnings blocking PRs.
🧪 Strategy: Adjusted `pub(crate)` to `pub` for `tools` and `experimental` modules, cleaned up unused components causing warnings in `experimental`.
🔬 Verification: `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`
