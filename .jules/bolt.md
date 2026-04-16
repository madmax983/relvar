## 2025-04-13 - [Performance optimizations for relational operators]
**Learning:** Checking cardinality (`self.cardinality()` vs `other.cardinality()`) allows iteration over the smaller `HashSet` when performing set-based `intersect` and `union` operations. Also, performing short-circuit optimization by checking `is_empty()` skips `.clone()` and allocation overheads completely when applying set `difference` and `intersect`.
**Action:** Always check relation size or employ fast-paths via `is_empty()` when doing set operations (especially O(N*M) or O(N) ops mapping to hash set operations) before allocating `HashSet`s or creating iterators.
## 2025-04-13 - [Performance optimizations for relational operators]
**Learning:** Checking cardinality (`self.cardinality()` vs `other.cardinality()`) allows iteration over the smaller `HashSet` when performing set-based `intersect` and `union` operations. Also, performing short-circuit optimization by checking `is_empty()` skips `.clone()` and allocation overheads completely when applying set `difference` and `intersect`.
**Action:** Always check relation size or employ fast-paths via `is_empty()` when doing set operations (especially O(N*M) or O(N) ops mapping to hash set operations) before allocating `HashSet`s or creating iterators.

## 2024-05-24 - [Optimization] Added extend_into for in-place relation extension
**Learning:** `Relation::extend` clones tuples unnecessarily when we could consume the original relation to extend it in place, avoiding redundant tuple allocation overhead.
**Action:** Implemented `extend_into` alongside `extend` to take ownership of tuples. This is part of the `_into` family optimization.
