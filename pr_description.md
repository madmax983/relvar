🎯 Target: `relvar_storage::wal::WalRecord::deserialize` in `relvar-storage/src/wal/record.rs`.
💣 Risk: Prevents potential unhandled panics or undefined behavior when provided with invalid or corrupted serialized log records (e.g. invalid postcard bytes) from disk.
🧪 Strategy: Added a test `test_wal_record_deserialize_errors` that asserts invalid serialized inputs correctly return a `WalRecordError::Serialization` rather than crashing the program.
🔬 Verification: `cargo test -p relvar-storage --lib wal::record::sentry_tests`
