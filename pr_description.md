💡 What: Replaced `.collect()` with `BTreeMap::from_iter` in `relvar-core/src/algebra/rename.rs` when initializing tuple mappings during attribute rename operations.

🎯 Why: Creating a `BTreeMap` from an iterator by calling `.collect()` inserts items individually, resulting in a series of `O(log N)` tree insertions. Since the input iterators (zipping old names with old values from an already sorted `BTreeMap`) are already perfectly sorted, using `BTreeMap::from_iter` allows standard library internals to perform an `O(N)` linear build of the tree. This reduces overhead, prevents intermediate sorting logic from being triggered, and minimizes internal branch mispredictions.

📊 Impact: Reduces computational complexity of generating new tuples during rename operations from `O(N log N)` to `O(N)`, especially beneficial when relations have a wide degree (many attributes).

🔭 Measurement: Run `cargo bench -p relvar-core --bench algebra rename` to verify the execution profile.
