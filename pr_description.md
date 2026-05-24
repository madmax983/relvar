# ⚡ Bolt: [TClose Rename Allocation Removal]

## 💡 What
In the transitive closure (`tclose`) algebraic operator, the accumulation loop was calling `r_delta.rename(delta_mappings)` during each iteration. This was replaced with `r_delta.rename_into(delta_mappings)` by transferring ownership, and the empty relation allocations used for `std::mem::replace` were hoisted outside the loop.

## 🎯 Why
`Relation::rename` allocates a new `Relation` and copies all tuple strings because it takes `&self`. By changing `compute_next_paths` to take `r_delta` by value rather than by reference, we can consume it using `.rename_into()`. The `rename_into` method checks if the strings have actually changed and reuses the old memory allocations if they match, which avoids repeated deep string allocations on every loop iteration. Pre-allocating the empty relations outside the loop avoids cloning the RelationType schema on every iteration.

## 📊 Impact
Removes the need to allocate and copy all attribute string names for each path processed in the transitive closure operator's hot path loop, turning an `O(N)` allocation per iteration step into a string-pointer reuse.

## 🔬 Measurement
Run `cargo bench tclose` and observe fewer heap allocations and better cache locality when computing transitive closures on deep graphs. Also verified via `cargo test tclose`.
