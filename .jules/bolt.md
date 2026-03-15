# Bolt's Journal ⚡

## 2024-05-22 - Storage Engine Bottleneck
**Learning:** `ConstraintManager` validation methods load entire relations into memory via `StorageEngine::load_relation` to perform foreign key checks. This creates `O(N)` memory usage and `O(N*M)` complexity for bulk operations.
**Action:** Future architectural improvements should extend `StorageEngine` to support granular existence checks (`contains_tuple`, `find_tuple`) or iterator-based access to avoid full loads.

## 2024-05-24 - Avoiding Clones in Update/Delete
**Learning:** `Relation::insert` takes `Tuple` by value. Iterating over `Relation` using `IntoIterator` allows moving tuples from old to new relation during updates/deletes, saving O(N) clones.
**Action:** When refactoring collection transformations, always check if the source collection can be consumed (`into_iter`) to avoid cloning elements.

## 2026-02-05 - Pre-allocation in Collection Transformations
**Learning:** Relational algebra operations often involve transforming one relation to another with preserved or predictable cardinality. Naively collecting into a `Vec` before building a `HashSet` (or other collection) incurs double allocation.
**Action:** Use `Iterator::size_hint` in constructors (like `from_tuples`) and pass iterators directly instead of intermediate collections. Add `with_capacity` constructors to custom collection types.

## 2026-02-05 - Pre-allocation in Hot Loops
**Learning:** Relational operators like `join`, `theta_join`, and `extend` often construct new tuples in tight loops. Pre-allocating the `HashMap` or `Vec` for the new tuple's attributes using `with_capacity(degree)` significantly reduces reallocation overhead when the resulting size is known.
**Action:** Always use `with_capacity` when constructing collections inside hot paths if the target size is known or can be estimated (e.g., from the relation heading).

## 2026-02-05 - Tuple Validation Overhead in Relational Operators
**Learning:** `Tuple::new` performs O(M) validation (where M is degree) for every tuple creation. In relational operators like `extend`, inputs are often already valid. Using `Tuple::new_unchecked` with manual checks for only new/modified attributes reduced execution time by ~35%.
**Action:** When implementing relational operators that derive new tuples from existing valid tuples, use `Tuple::new_unchecked` and manually validate only the changed parts.

## 2026-02-05 - Zero-Allocation Keys in Semijoin
**Learning:** `semijoin` and `semidifference` operations allocated a `Vec<&ScalarValue>` for every tuple in the right-hand relation to perform hash lookups. This O(M) allocation overhead dominated performance for small relations.
**Action:** Replaced `Vec` keys with a `SemijoinKey` wrapper struct that holds references to the tuple and attributes, enabling zero-allocation hashing and comparison. This yielded an ~8% speedup for small relations.

## 2024-03-01 - [Zero-Copy Primitive Extraction]
**Learning:** `Tuple::get_typed<T>` was invoking `.clone()` on the `ScalarValue` enum purely to satisfy the `TryFrom<ScalarValue>` trait bound, causing a 24-byte enum copy for every extraction.
**Action:** Changed trait bound to `T: for<'a> TryFrom<&'a ScalarValue>` to enable extraction by reference and avoid the enum wrapper clone, while preserving type safety.

## 2026-03-06 - [Zero-Copy String Extraction]
**Learning:** `Tuple::get_typed<T>` was invoking `.clone()` on the `ScalarValue::String` enum purely to satisfy the `TryFrom<&'a ScalarValue> for String` trait bound. This caused unnecessary heap allocations when borrowing as `&str` was sufficient.
**Action:** Implemented `TryFrom<&'a ScalarValue> for &'a str` to enable zero-copy string extraction via `Tuple::get_typed::<&str>`, saving a heap allocation on every hot-path string extraction.
## 2026-03-08 - Tuple Pre-allocation in Extend
**Learning:** The `extend` operator iterates over `self.tuples()` and builds a completely new `Vec<Tuple>` by calling `collect()`. However, we know that the cardinality of an extended relation will always exactly match the cardinality of the input relation, because `extend` adds exactly one attribute to every existing tuple without adding or removing any tuples.
Without pre-allocating the vector for `extended_tuples` with `with_capacity()`, the underlying vector will repeatedly reallocate as tuples are computed and pushed onto it, which is especially noticeable for large relations.

**Action:** When implementing operations like `extend` or `project` that have a 1:1 input-to-output tuple mapping or have a known upper bound, use an iterator with a `size_hint` or explicitly allocate `Vec::with_capacity(self.cardinality())` before iterating, or verify that the returned iterator properly implements `size_hint` so that `collect()` can optimize the allocation.

## 2026-03-08 - Tuple Pre-allocation and Validation bypass in Extend
**Learning:** The `extend` operator previously allocated a `Vec` for intermediate tuple storage and then used `Relation::from_tuples` which reallocated into a `HashSet` and performed O(N*M) validation against the heading for every tuple, despite the tuples already being valid by construction.
**Action:** Iterate directly into a pre-allocated `HashSet::with_capacity(self.cardinality())` to avoid the intermediate `Vec` allocation, and use `Relation::from_tuples_unchecked` to safely bypass redundant validation overhead when tuples are known to conform to the new heading.
