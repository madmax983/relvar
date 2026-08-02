Title: ⚡ Bolt: [Divide Operator Allocation Removal]
💡 What: Optimized the `divide` operator's `BTreeMap` extension step by directly cloning `candidate.values()` and using `.extend()` instead of relying on iterator chaining and `.collect()`.
🎯 Why: Iterator chaining and `.collect()` to construct a new `BTreeMap` from existing ones introduces significant overhead because it processes each element one by one, allocating memory repeatedly and potentially causing re-hashing logic during insertion.
📊 Impact: Considerably faster `divide` executions by avoiding redundant intermediate memory allocations and utilizing optimized `BTreeMap` cloning mechanisms.
🔬 Measurement: Run `cargo bench --bench divide_perf` before and after the change.
