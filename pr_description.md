⚡ Bolt: HashSet Pre-Allocation in Relation::from_tuples

💡 **What**: Updated `Relation::from_tuples` to pre-allocate its internal `HashSet` using the upper bound of the `size_hint` iterator when available (capped to avoid memory bombs), rather than strictly using the lower bound.

🎯 **Why**: When chaining iterators (such as mapping or filtering), iterators often have a lower bound of `0` and an upper bound that exactly matches the source structure's size. Pre-allocating using the lower bound (`0`) causes the `HashSet` to repeatedly reallocate and resize as elements are sequentially inserted into it. Safely using the upper bound (when `< 100,000` to prevent OOM vulnerabilities) minimizes hashing and reallocation churn.

📊 **Impact**: Reduces implicit heap reallocations in collection constructions across the workspace that use standard `from_tuples`.

🔬 **Measurement**: Standard memory allocation structural improvement, mitigates `clippy` capacity warnings indirectly.
