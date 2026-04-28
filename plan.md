1. **Analyze existing dependencies**: Look through `relvar-core` and see that `pub mod`s were previously leaking encapsulation and creating a flat public API with circular loops, which violates boundary encapsulation rules. I will fix the remaining modules to `pub(crate)` to correctly enforce boundaries.
2. **Remove the God File `relvar-storage/src/storage/heap/tests/scan.rs`**: This file is 1200+ lines. It should be split into smaller test files, such as `scan_visible.rs`, `scan_corruptions.rs`, `scan_security.rs` inside `relvar-storage/src/storage/heap/tests/` to prevent "The Blob" smell.
3. **Write changes to journal**: Document this extraction in `.jules/atlas.md`
4. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
5. **Submit**: After running verification, submit the changes as a PR for Atlas.
