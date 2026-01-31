## 2025-02-18 - Arbitrary Limits in Loops
**Learning:** Found a hardcoded `if page_id > 1000` break in `HeapFile::scan`, intended as a safety guard but acting as a severe data truncation bug for datasets > 4MB.
**Action:** Always scrutinize "magic numbers" in loop termination conditions. Test boundaries explicitly (e.g., if limit is 1000, test 1001).

## 2025-02-18 - Silent Corruption Masking in Page Read
**Learning:** `PageFile::read_page` was returning an empty valid page when encountering corrupted length prefixes or partial reads, potentially masking data corruption as "no data".
**Action:** When reading length-prefixed data, always validate that the declared length matches the available buffer size. Explicitly handle EOF vs Partial/Corrupted reads.

## 2026-01-31 - Silent Error Swallowing in Collection Iterators
**Learning:** `HeapFile::scan` was silently ignoring deserialization errors during iteration, effectively hiding data corruption from the caller.
**Action:** Avoid `filter_map` or swallowing `Result::Err` in core storage loops. Always propagate serialization errors up to the caller to fail fast on corruption.
