🎯 **Target:** Added test coverage for internal Heapfile utilities in `relvar-storage/src/storage/heap/mod.rs` and missing error branch mapping logic for `_into` algebra functions (`extend_into`, `difference_into`, `intersect_into`, `union_into`) and `Delta::compose` in `relvar-core/src/algebra/`.

💣 **Risk:** Untested boundary conditions for `extract_tuples_from_slots` and `extract_all_tuples` in `relvar-storage` could hide page/memory slicing and bounds checking issues. In `relvar-core`, missing error checks in `_into` variants might panic on type mismatch errors rather than returning properly wrapped Domain errors.

🧪 **Strategy:** Created a new file `relvar-core/tests/sentry_algebra_into_coverage.rs` to assert explicit `Err(...)` propagation on structural mismatch combinations for algebra. Created targeted tests in `relvar-storage/src/storage/heap/tests/other.rs` simulating correctly-sized slot inputs mapped over manually-populated heap storage entries to guarantee successful reads over `extract_tuples_from_slots` and `extract_all_tuples`.

🔭 **Verification:** `cargo test -p relvar-core --test sentry_algebra_into_coverage` and `cargo test -p relvar-storage test_sentry_extract`
