Title: 🛡️ Sentry: [Tuple Accessors and Serde Coverage]

🎯 Target: `Tuple` accessors, `TupleError` displays, and `arc_serde` proxy in `relvar-core/src/values/tuple.rs`.
💣 Risk: Missing test coverage for basic getters, serialization round-trips, and error string formatting.
🧪 Strategy: Added tests for debug representations, clone behaviors, degree mismatches in `conforms_to`, and JSON roundtripping over Arc proxies.
🔬 Verification: `cargo test --test sentry_tuple_coverage`
