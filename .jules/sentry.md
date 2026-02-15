## 2025-02-18 - Arbitrary Limits in Loops
**Learning:** Found a hardcoded `if page_id > 1000` break in `HeapFile::scan`, intended as a safety guard but acting as a severe data truncation bug for datasets > 4MB.
**Action:** Always scrutinize "magic numbers" in loop termination conditions. Test boundaries explicitly (e.g., if limit is 1000, test 1001).

## 2025-02-18 - Silent Corruption Masking in Page Read
**Learning:** `PageFile::read_page` was returning an empty valid page when encountering corrupted length prefixes or partial reads, potentially masking data corruption as "no data".
**Action:** When reading length-prefixed data, always validate that the declared length matches the available buffer size. Explicitly handle EOF vs Partial/Corrupted reads.

## 2026-01-31 - Silent Error Swallowing in Collection Iterators
**Learning:** `HeapFile::scan` was silently ignoring deserialization errors during iteration, effectively hiding data corruption from the caller.
**Action:** Avoid `filter_map` or swallowing `Result::Err` in core storage loops. Always propagate serialization errors up to the caller to fail fast on corruption.

## 2026-02-18 - Silent Corruption Healing in Storage Operations
**Learning:** `HeapFile` modification methods (`insert`, `update`, `delete`) were silently swallowing corrupted slots (pointing outside page bounds) by replacing them with empty tuples during page rewrite. This effectively "healed" the page by deleting the inaccessible data without warning.
**Action:** In storage modification paths, always treat structural corruption (e.g., out-of-bounds pointers) as a hard error. Never "skip" or "default" corrupted data during a rewrite, as this makes data loss permanent.

## 2026-05-21 - Silent Data Loss in Rename Collision
**Learning:** `Relation::rename` handles attribute collisions (multiple attributes renamed to same target) by iterating map entries. Due to `BTreeMap` order, the lexicographically last attribute overwrites earlier ones without warning.
**Action:** When testing renaming or mapping operations, always verify "collision" scenarios. Document the behavior explicitly in tests to prevent accidental regression if iteration order changes.

## 2026-06-15 - Unnecessary Integer Overflow in Aggregation
**Learning:** `Avg` aggregation was using `i64` accumulator, causing it to fail on large datasets where the sum exceeds `i64::MAX` even if the average is small.
**Action:** Use `i128` (or larger type) accumulators for integer aggregations to prevent intermediate overflows.

## 2026-10-24 - Inconsistent NaN Handling in Scalar Values
**Learning:** Found that  relied on  for equality and hashing, causing different NaN payloads to be treated as distinct values (and distinct groups in ).
**Action:** When implementing database types, always normalize NaNs in , , and  to ensure set semantics (all NaNs are equal), regardless of the underlying bit pattern.

## 2026-10-24 - Inconsistent NaN Handling in Scalar Values
**Learning:** Found that `ScalarValue::Float` relied on `f64::to_bits()` for equality and hashing, causing different NaN payloads to be treated as distinct values (and distinct groups in `summarize`).
**Action:** When implementing database types, always normalize NaNs in `Eq`, `Hash`, and `Ord` to ensure set semantics (all NaNs are equal), regardless of the underlying bit pattern.

## 2027-02-27 - Slot Reuse Data Corruption
**Learning:** `HeapFile::try_insert_into_page` (and versioned variants) caused data corruption when reusing a freed slot (e.g. from deletion). It was calling `vec.insert()` which shifts subsequent elements, but the `slots` vector was not shifted (since we reused an index). This misaligned slots and tuple data, causing subsequent tuples to be lost or corrupted during page repack.
**Action:** When managing parallel vectors (slots and data), ensure modifications are symmetric. If reusing a slot index, use index assignment (`vec[i] = val`) instead of insertion (`vec.insert(i, val)`).
