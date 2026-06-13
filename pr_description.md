🪒 Razor: [Reduction] Remove dead/speculative code

**Bloat:**
- The `Delta` module in `relvar-core/src/algebra/delta.rs` was marked with `#[allow(dead_code)]` and completely unused by the application.
- The `target_attr` field in `NaiveBayesClassifier` (`relvar/src/experimental/automl.rs`) was unused.

**Cut:**
- Deleted `relvar-core/src/algebra/delta.rs` and its dedicated test file `relvar-core/tests/sentry_delta_coverage.rs`.
- Removed `Delta` exports from `relvar-core/src/algebra/mod.rs`.
- Cleaned up unused variables and tests associated with these features across the codebase.

**Saved:**
- ~500 lines of dead code and tests removed.
