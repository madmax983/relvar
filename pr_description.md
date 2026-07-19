🎸 Bard: Fix CI failures by restoring module visibility and unused imports

📖 Chapter: `Visibility and Imports`
🔦 Insight: The CI tests `warden_exploit_csv_dos` and `warden_json_import` were failing due to the module `tools` in `relvar/src/lib.rs` and the enum `ImporterError` in `relvar/src/tools/importer.rs` being restricted to `pub(crate)` instead of `pub`. Similarly, the storage tests failed because `RelationMetadata`, `PAGE_SIZE`, and `PageId` were not publicly re-exported or allowed to be unused in `relvar-storage/src/storage/mod.rs`.
🧪 Example: Adjusted module visibility and allowed unused imports to satisfy integration tests.
