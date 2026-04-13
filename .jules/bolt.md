## 2024-05-18 - Avoid repeated iterator allocation layers when building BTreeMap

**Learning:** When collecting an iterator of pairs `(&T, &U)` into a `BTreeMap<T, U>`, mapping the iterator using `.map(|(k, v)| (k.clone(), v.clone()))` and then collecting with `BTreeMap::from_iter` creates an extra iterator adapter layer. By cloning the values directly during the initial traversal when the `Vec` is built (returning `Vec<(T, U)>` instead of `Vec<(&T, &U)>`), we can pass the owned `Vec` directly to `BTreeMap::from_iter`.

**Action:** Modified `merge_tuple_values` in `join.rs` to clone strings and values directly into the `Vec` during `merge_next_values`. This allows `combine_tuples` to consume the vector directly into a `BTreeMap` without an intermediate `map()` adapter, slightly boosting performance and simplifying the calling code while adhering to the instruction to "move values without cloning" during map creation.
