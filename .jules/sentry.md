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
