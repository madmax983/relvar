🎯 Target: `relvar-core/src/database` module
💣 Risk: Virtual relvars modification error coverage, tuple mismatch error coverage, and getters test coverage on missing constraint manager wrappers.
🧪 Strategy: Added `get_type_constraints` and `get_check_constraints` wrappers to the `ConstraintManager` and `Database` so that properties correctly bubble up via getters for tests. Implemented explicit matching tests for error paths that Tarpaulin showed were omitted.
🔬 Verification: Run `cargo test` in `relvar-core`.
