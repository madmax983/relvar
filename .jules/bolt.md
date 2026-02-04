# Bolt's Journal ⚡

## 2024-05-22 - Storage Engine Bottleneck
**Learning:** `ConstraintManager` validation methods load entire relations into memory via `StorageEngine::load_relation` to perform foreign key checks. This creates `O(N)` memory usage and `O(N*M)` complexity for bulk operations.
**Action:** Future architectural improvements should extend `StorageEngine` to support granular existence checks (`contains_tuple`, `find_tuple`) or iterator-based access to avoid full loads.

## 2024-05-24 - Avoiding Clones in Update/Delete
**Learning:** `Relation::insert` takes `Tuple` by value. Iterating over `Relation` using `IntoIterator` allows moving tuples from old to new relation during updates/deletes, saving O(N) clones.
**Action:** When refactoring collection transformations, always check if the source collection can be consumed (`into_iter`) to avoid cloning elements.
