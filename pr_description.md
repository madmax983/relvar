# 📚 Bard: [documentation update]

📖 Chapter: Added `/// # Examples` tests to `relvar/src/experimental/spreadsheet.rs` and fixed empty doc tests for `relvar-core/src/utils/recursion.rs`. Also fixed the `pub(crate)` module warnings in `relvar` and `relvar-storage`. Finally, fixed unused import and unused code warnings in tests.
💡 Insight: Module-level documentation and executable doctests clarify how the Spreadsheet component works and ensures internal code examples don't fail parsing. Tests were failing because `storage` and `persistent_engine` modules in `relvar-storage` and `experimental` and `tools` in `relvar` were private.
🧪 Example: Added an example showing how to evaluate `B1 = A1 / A2` relationally.
🖼️ Preview: Documentation generates successfully with zero warnings.
