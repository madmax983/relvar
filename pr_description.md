Title: 🎻 Bard: [documentation update]

📖 Chapter: The `tools` module in `relvar`.
🔦 Insight: Removed unused items `RelationMetadata`, `PAGE_SIZE`, and `PageId` from `relvar-storage/src/storage/mod.rs` to fix clippy warnings. We also ensured the tools module was exported correctly, allowing tests to run properly without `E0603: module tools is private` error.
🧪 Example: The build and all tests pass locally.
