🎯 Target: `relvar-storage/src/storage/heap.rs` internal serialization methods and boundaries (`check_tuple_size_limit`, `verify_versioned_page_size`, `find_page_for_insertion`, `read_tuple` on empty pages)
💣 Risk: Missing test coverage for maximum capacity sizing and buffer limits could obscure memory allocation regressions.
🧪 Strategy: Added `sentry_coverage.rs` and `sentry_coverage2.rs` unit tests simulating extreme limit values (`u32::MAX`) to explicitly trigger `HeapError::Serialization` and `HeapError::PageFull` error mappings.
🔭 Verification: `cargo test -p relvar-storage --lib -- test_sentry`
