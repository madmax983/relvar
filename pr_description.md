# 🛠️ Forge: Fix CI failures and test issues

🎯 **Target:** Multiple files across `relvar`, `relvar-storage`, and `relvar-core`.
💣 **Risk:** The previous commit introduced visibility issues (`pub(crate)` instead of `pub` for `experimental` and `tools` in `relvar` and `persistent_engine` and `storage` in `relvar-storage`), unused code, type annotation errors in benches, and doctest failures in `relvar-storage/src/storage/page.rs`.
🧪 **Strategy:** Changed `pub(crate)` to `pub` for the relevant modules. Fixed unused code and type annotations in benches. Fixed failing doctests in `relvar-storage/src/storage/page.rs` to allow checks to pass.
🔬 **Verification:** Ran `cargo test --all-features`, `cargo test -p relvar-storage --doc`, `cargo test --manifest-path relvar-storage/Cargo.toml --benches`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check`. All checks passed.
