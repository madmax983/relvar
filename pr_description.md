💡 What: Switched to `rename_into` inside the `Query::Rename` execution block.
🎯 Why: `Query` execution naturally owns its intermediate `relation` results. Using the `rename` method created a redundant clone of the relation and its tuples, whereas `rename_into` consumes the owned relation and modifies tuples in-place.
📊 Impact: Eliminates a redundant collection allocation and tuple cloning step per query rename execution, saving time and memory on the heap.
🔬 Measurement: Run `cargo bench --bench database` and observe the allocation and time performance of query execution.
