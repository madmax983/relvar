# ⚒️ Forge: Fix CI visibility issue

🚮 Smell: Multiple integration tests were failing because `tools` module in `relvar/src/lib.rs` was marked as `pub(crate)` instead of `pub`, hiding it from the `tests/` directory which are external to the crate. Unused imports in `relvar-storage/src/storage/mod.rs` were also triggering a clippy warning.
✨ Solution: Changed `pub(crate) mod tools;` to `pub mod tools;` in `relvar/src/lib.rs` and removed the unused imports in `relvar-storage`.
🧼 Benefit: Resolves the CI failures and enforces idiomatic visibility.
🛡️ Verification: Tests passed. No logic changed.
