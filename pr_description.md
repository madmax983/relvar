🛡️ Sentry: [test coverage improvement]

🎯 Target: `relvar-core/src/algebra/join.rs`
💣 Risk: Missing validation for attributes during internal join loops could lead to panics if unhandled.
🧪 Strategy: Added coverage for `ok_or_else` paths where `Tuple::get(attr)` fails in build and probe phases using corrupted tuples.
🔬 Verification: `cargo test -p relvar-core algebra::join::sentry_coverage_tests`
