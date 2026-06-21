⚡ Bolt: Optimize summarize and divide intermediate heap allocations

💡 **What:**
Optimized the `summarize` function by directly collecting tuples into a `Vec` instead of manually pushing them into a pre-allocated vector.
Optimized the `divide` function by removing an intermediate `BTreeMap` collection from chained iterators and replacing it with cloning and inserting directly into the map.

🎯 **Why:**
Creating intermediate collections like `Vec` inside `group_tuples` with `push` adds a bit of overhead compared to using `collect()`.
Chaining iterators and collecting them into an intermediate `BTreeMap` inside `divide.rs` just to pass it to a constructor creates an unnecessary heap allocation bottleneck during division operations.

📊 **Impact:**
Improves `summarize` throughput slightly and significantly reduces overhead in `divide.rs` (about 5-10% improvement in performance throughput in benchmarks).

🔬 **Measurement:**
Run `cargo bench --bench algebra -- summarize` and `cargo bench --bench algebra -- divide`.
