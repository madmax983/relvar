1.  **Refactor `recover` in `relvar-storage/src/wal/recovery.rs`:**
    *   The `recover` function is 61 lines long and contains a mix of finding the max transaction ID, collecting active transactions, processing committed/aborted transactions, and collecting uncommitted inserts.
    *   Extract finding the max transaction id and active transactions into a helper function `identify_active_and_max_txn`.
    *   Extract the uncommitted insert logic into a helper `collect_uncommitted_inserts`.
2.  **Verify compilation and tests:**
    *   Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
3.  **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
4.  **Submit the PR:**
    *   Create PR with Title: "⚒️ Forge: [Refactor `recover` in `relvar-storage/src/wal/recovery.rs`]"
    *   Description with Smell, Solution, Benefit, Verification.
