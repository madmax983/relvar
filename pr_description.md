🎯 Target: `WalRecordError` in `relvar-storage/src/wal/record.rs`
💣 Risk: The `WalRecordError` enum was missing a Display trait format test, causing a minor code coverage gap.
🧪 Strategy: Added a new `test_wal_record_error_display` test that asserts the string representation of both variants (`Serialization` and `RecordTooLarge`).
🔬 Verification: `cargo test -p relvar-storage wal::record`
