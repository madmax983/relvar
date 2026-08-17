# ⚒️ Forge: Refactor `recover` function in `relvar-storage/src/wal/recovery.rs`

🚮 Smell: The `recover` function in `relvar-storage/src/wal/recovery.rs` was 61 lines long and featured a complex, imperative style of mutating state and filtering data. It was unnecessarily verbose, modifying `active_txns` and tracking `max_txn_id` using nested mutable references and manual loops.

✨ Solution: I refactored the function to use idiomatic Rust iterator chains. `max_txn_id` is now computed with `.iter().filter_map().max()`. `active_txns` uses `.iter().filter_map().collect()` and `.retain()`. Finally, `uncommitted_inserts` is built with a single `.iter().filter_map().collect()` pipeline.

🧼 Benefit: Dramatically improves readability, makes data flow explicitly clear, and removes multiple mutable state variables (`active_txns` mutation is minimized and `max_txn_id` is calculated directly).

🛡️ Verification: Tests passed. No logic changed.
