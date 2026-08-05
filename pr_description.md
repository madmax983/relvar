Title: 🛡️ Sentry: [test coverage improvement]

🎯 Target: Tuple public methods and WalRecordError display.
💣 Risk: Tuple internal operations `into_values` and `attribute_names` had zero unit test coverage for correctness. `WalRecordError` derived display method wasn't tested for correct string mappings in storage layer logic.
🧪 Strategy: Added explicit validation tests asserting exact map conversions and iteration ordering. Appended `test_wal_record_error_display` to cover `RecordTooLarge` boundary and postcard serialization propagation errors natively.
🔬 Verification: cargo test
