# Bolt's Journal ⚡

## 2024-05-22 - Storage Engine Bottleneck
**Learning:** `ConstraintManager` validation methods load entire relations into memory via `StorageEngine::load_relation` to perform foreign key checks. This creates `O(N)` memory usage and `O(N*M)` complexity for bulk operations.
**Action:** Future architectural improvements should extend `StorageEngine` to support granular existence checks (`contains_tuple`, `find_tuple`) or iterator-based access to avoid full loads.
