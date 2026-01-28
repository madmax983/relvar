## 2025-02-18 - Arbitrary Limits in Loops
**Learning:** Found a hardcoded `if page_id > 1000` break in `HeapFile::scan`, intended as a safety guard but acting as a severe data truncation bug for datasets > 4MB.
**Action:** Always scrutinize "magic numbers" in loop termination conditions. Test boundaries explicitly (e.g., if limit is 1000, test 1001).
