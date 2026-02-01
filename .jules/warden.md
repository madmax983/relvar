## 2024-05-23 - HeapFile Error Handling and KeyConstraints Panic
**Threat:**
1. `HeapFile` operations (`insert_tuple`, `scan`) were masking I/O errors (e.g., permission denied, hardware failure) by treating them as EOF/New Page.
2. `KeyConstraints::would_violate` could panic if passed a tuple missing a key attribute.

**Defense:**
1. Modified `HeapFile::try_insert_into_page` and `scan` to propagate errors from `read_page` using `?`. Note: `read_page` handles EOF by returning `Ok(empty_page)`, so propagating `Err` correctly surfaces *real* I/O errors without breaking page growth.
2. Modified `KeyConstraints::extract_key_value` to return `Result` and added `KeyConstraintError::TupleMissingAttribute`. Updated call sites to propagate this error.

## 2024-05-24 - HeapFile DoS via Infinite Loop
**Threat:**
`HeapFile::insert_tuple` entered an infinite loop when attempting to insert a tuple larger than `PAGE_SIZE`. The loop continuously allocated new empty pages in memory (via `read_page` on incrementing IDs) trying to find one that fit, leading to CPU exhaustion (DoS).

**Defense:**
Added a pre-check `check_tuple_size_limit` using `bincode::serialized_size` on a dummy page to verify if the tuple can theoretically fit in an empty page. If it exceeds capacity, it returns `HeapError::TupleTooLarge` immediately.
