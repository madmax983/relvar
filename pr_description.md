# ⚡ Bolt: [performance improvement]

💡 **What:** Changed `active_txn_table.rs` and `snapshot.rs` to allocate active transactions directly into a `std::collections::HashSet` instead of collecting them into an intermediate `Vec` first.

🎯 **Why:** Creating a `TransactionSnapshot` is a very hot path during concurrent transactions. Collecting `TransactionId`s into a `Vec` and then passing it into `TransactionSnapshot::new` (which then iterates and collects it into a `HashSet`) introduces an entirely redundant heap allocation every time a new transaction begins.

📊 **Impact:** Reduces heap allocations by 1 per new transaction started. The intermediate `Vec` allocations and subsequent element copies have been completely eliminated.

🔬 **Measurement:** Verify by looking at test performance or running existing benchmarks.
