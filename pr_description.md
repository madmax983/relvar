🛡️ Sentry: [test coverage improvement]

🎯 Target: `relvar_storage::wal::record::WalRecord::is_txn_end`
💣 Risk: Hidden false-positive matches on state check logic in critical WAL records.
🧪 Strategy: Added explicit test cases verifying that `PageWrite`, `Delete`, and `Checkpoint` correctly return false for `is_txn_end()`.
🔬 Verification: Run `cargo test --lib --manifest-path relvar-storage/Cargo.toml -- test_is_txn_end_other_variants`.
