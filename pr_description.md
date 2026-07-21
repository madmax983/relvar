🎯 Target: `ScalarType::cmp` logic for `UserDefined` types, `RelationType::new` depth check, `TupleType::with_attribute` depth check, in `relvar-core`.

💣 Risk: Untested boundaries and unverified state comparisons. The depth limits on nested scalar types prevent stack overflows, but were missing edge-case verification for relation and tuple type constructors. User-defined types needed order verification to prevent panics during comparisons or BTreeMap insertions.

🧪 Strategy:
- Added tests to trigger the `Type nesting too deep` panic on boundary values for `RelationType::new` and `TupleType::with_attribute`.
- Added tests to cover `cmp_user_defined_types` to verify proper ordering behavior for `UserDefined` representations.

🔬 Verification: `cargo test --package relvar-core`
