# 🛡️ Sentry: [test coverage improvement]

🎯 Target: `extract_tuples_from_versioned_slots` and `validate_slot_bounds` in `relvar-storage/src/storage/heap/mod.rs`
💣 Risk: Prevents potential panics or buffer overflows during MVCC page extraction by explicitly testing the bounds validation logic with maliciously corrupted page offset data.
🧪 Strategy: Added specific edge-case unit tests in `relvar-storage/src/storage/heap/tests/corruption.rs` to simulate out-of-bounds offsets and integer overflows inside versioned slot metadata.
🔬 Verification: `cargo test --package relvar-storage`
