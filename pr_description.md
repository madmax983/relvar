Title: 🛡️ Sentry: ScalarValue test coverage improvement

🎯 Target: relvar-core/src/values/scalar.rs
💣 Risk: Missing test coverage for Hash, PartialOrd, and Cmp implementations on Bool, Bytes, Relation, and UserDefined variants.
🧪 Strategy: Added specific unit tests to exercise Eq, Hash, and Ord logic for these variants.
🔬 Verification: cargo test --test sentry_scalar_value_coverage
