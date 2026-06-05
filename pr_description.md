# 🛡️ Sentry: [test coverage improvement]

🎯 **Target:** `relvar-core/src/database/dml.rs` (DML Update duplicates logic) and `relvar-storage/src/wal/record.rs` (Record serialization/deserialization limits/edge logic)

💣 **Risk:** The core DML update logic lacked explicit fallback checks triggering correctly evaluated panics under specific deduplication mappings against candidate keys when sets overlap. The internal `WalRecord` bounds validation for limit capacities specifically during `deserialize` and `serialize` could panic/allow silent allocation limits if not covered explicitly inside its boundaries.

🧪 **Strategy:** Added explicit unit test boundaries verifying DML updates that violate primary keys during a rewrite throw the properly routed enum `ConstraintManagerError::CandidateKeyViolation` instead of silently erasing the cardinality, safely enforcing RM proscription logic. Also added specific `is_txn_end`, `txn_id`, `serialize`, and `deserialize` function coverage to `relvar-storage/src/wal/record.rs`.

🔬 **Verification:** `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`
