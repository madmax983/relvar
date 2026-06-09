🛡️ Sentry: [test coverage improvement]

🎯 Target: `compute_relation_after_delete` and `compute_relation_after_update` in `relvar-core/src/database/dml.rs`.
💣 Risk: Untested core data manipulation logic. Without coverage, changes to tuple reduction logic, type matching, or subset merging could introduce serious database inconsistencies or constraint bypasses without failing the build.
🧪 Strategy: Added internal unit tests to verify:
- Success paths for targeted deletes and updates.
- Proper calculation of modified tuple counts.
- `DatabaseError::TupleMismatch` when an updater returns a tuple that violates the schema constraints.
- Correct set-theoretic behavior (cardinality reduction) when updating causes tuples to merge into identical entities.
🔬 Verification: Run `cargo test -p relvar-core dml`
