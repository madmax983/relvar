💡 What: Optimized `rename_into` inside `relvar-core/src/algebra/rename.rs` by re-using the existing String memory allocation when an attribute name does not change.
🎯 Why: `rename_into` previously extracted the `values` from the tuple, discarded the original String keys via `.into_values()`, and explicitly allocated a new cloned string for every single attribute column of every single tuple using `new_name.clone()`. For relations with a large degree or high tuple count where only one or two fields are being renamed, this resulted in an enormous amount of useless String allocations.
📊 Impact: Avoids `M * N` String allocations (where M is un-renamed attributes per tuple, and N is the number of tuples).
🔬 Measurement: Run tests via `cargo test`.
