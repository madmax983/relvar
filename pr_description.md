Title: 🛡️ Sentry: [test coverage improvement]

🎯 Target:
- `relvar-core/src/values/tuple.rs` (TryFrom traits)
- `relvar-core/src/values/scalar.rs` (Cmp, Hash for types and values)
- `relvar-core/src/types/scalar.rs` (Cmp logic)
- `relvar-core/src/query/mod.rs` (Query variants: Join, Rename, Project, Summarize)

💣 Risk:
- Silent panic/logic failure when converting types or extracting tuples without tests.
- Undetected ordering errors in scalar value comparisons.
- Misexecution of query plan operators resulting in bad state.

🧪 Strategy:
- Added explicit unit tests to ensure `TryFrom` handles wrong types cleanly as `Err(())`.
- Evaluated nested hashing and type ordering mechanisms for `ScalarValue` and `ScalarType` (especially nested structures like UserDefined and Relation).
- Exercised all query operators (`Query::Join`, `Query::Rename`, `Query::Project`, `Query::Summarize`) against `InMemoryEngine` to ensure parsing and execution works as intended.

🔭 Verification:
`cargo test`
