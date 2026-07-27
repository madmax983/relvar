Title: 🛡️ Sentry: [test coverage improvement]

🎯 Target: `intersect_into`, `union_into`, and `difference_into` in `relvar-core/src/algebra/intersect.rs`, `relvar-core/src/algebra/union.rs`, and `relvar-core/src/algebra/difference.rs`.
💣 Risk: Untested error branches for type mismatch could lead to unexpected behavior if not mapping to the correct error types when relations have different headings.
🧪 Strategy: Added specific unit tests in `relvar-core/tests/sentry_difference_intersect_into_test.rs` to verify that `intersect_into`, `union_into`, and `difference_into` correctly return an error on type mismatch.
🔬 Verification: `cargo test --test sentry_difference_intersect_into_test`
