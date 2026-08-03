Title: 🔒 Warden: Fix experimental module visibility issue

🦠 Threat: Broken builds downstream when attempting to run tests, caused by `experimental` being `pub(crate)` and `tools` being `pub(crate)` instead of `pub mod` preventing cross-crate access from the test suite.
🛡️ Defense: Changed `pub(crate) mod experimental` to `pub mod experimental` and `pub(crate) mod tools` to `pub mod tools` in `relvar/src/lib.rs`. Reverted `storage` to `pub mod storage` in `relvar-storage/src/lib.rs` to fix `relvar-storage` tests.
💥 Severity: Low - This is a compilation issue breaking tests, not a live exploit.
🧪 Verification: Scanned with `cargo clippy` and ran `cargo test` successfully across the workspace.
