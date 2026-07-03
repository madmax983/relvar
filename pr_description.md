🛡️ Sentry: [test coverage improvement]

🎯 Target: `relvar-storage/src/storage/page.rs`, `relvar-storage/src/storage/manager.rs`, `relvar-storage/src/storage/heap/mod.rs`
💣 Risk: Missing test coverage for error conditions like `PageTooLarge`, parsing corrupted data, I/O errors, and corrupted pages in heap structures which can lead to panics if unhandled.
🧪 Strategy: Added unit tests exercising the error paths explicitly via manipulating bounds or constructing malformed structs (e.g., corrupted length prefixes, slot directory overflows).
🔬 Verification: `cargo test`
