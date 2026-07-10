🎯 Target: `relvar-storage` components (`storage/manager.rs`, `storage/catalog.rs`, `storage/page.rs`, `mvcc/active_txn_table.rs`)
💣 Risk: Missing test coverage for error path conversions, bounds checks handling serialization edge-cases, directory I/O creation logic, and default initialization blocks which could obscure bugs.
🧪 Strategy: Added multiple direct component unit tests, and refactored some provably unreachable boundaries to align structural representation with logic flow.
🔭 Verification: Run `cargo test -p relvar-storage` and `cargo tarpaulin` (if installed) to observe improved coverage.
