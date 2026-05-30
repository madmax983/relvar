🎯 Target:
`Relation::extend_into` error conditions in `relvar-core/src/algebra/extend.rs`.

💣 Risk:
Unexercised error paths when `extend_into` receives an existing attribute name or when the computed value has an incorrect scalar type compared to the declared target type, leading to possible untested failures in consuming-mode logic.

🧪 Strategy:
Added explicit unit tests mirroring `extend` coverage by passing invalid inputs to `extend_into` and correctly unwrapping/matching the `ExtendError::AttributeExists` and `ExtendError::TupleCreation` enumerations.

🔬 Verification:
Run `cargo test --test sentry_extend_into_coverage` and verify coverage metrics for `extend_into` have improved.
