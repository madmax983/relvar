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

## 2024-05-25 - PageFile Integer Overflow
**Threat:**
1. `PageFile::read_page`, `write_page`, and `write_page_buffered` calculated file offsets as `page_id * PAGE_SIZE`, allowing integer overflow if `page_id` is large enough. This could lead to reading/writing from the wrong location or wrapping around to the beginning of the file.
2. `PageFile::read_page` checked `data_len + 8 > buffer.len()` without overflow protection. A crafted page with `data_len` near `usize::MAX` could bypass this check, leading to buffer overflow or panic when slicing `buffer[8..8+data_len]`.

**Defense:**
1. Replaced `*` with `checked_mul` for offset calculations, returning `PageError::PageTooLarge` on overflow.
2. Replaced `+` with `checked_add` for `data_len` check, returning `PageError::Serialization` on overflow.

## 2026-02-03 - HeapFile Integer Overflow and Deserialization Hardening
**Threat:**
1. `HeapFile::extract_tuple_from_page` and `extract_raw_tuple_data` calculated `start + length` without overflow protection. On 32-bit systems, this could wrap around, bypassing the `end > page.data().len()` check and causing out-of-bounds access or panic.
2. `HeapFile::deserialize_versioned_page` assumed `page.data()` had at least 5 bytes if the first byte matched the version, potentially panicking on short malicious pages.
3. `HeapFile::serialize_versioned_page_with_tuples` did not verify that `slot_dir.len()` fit in `u32`, potentially truncating the length prefix.

**Defense:**
1. Replaced `+` with `checked_add` in offset calculations, returning `HeapError::Serialization` on overflow.
2. Added explicit length check in `deserialize_versioned_page` to ensure the header exists before slicing.
3. Added check for `slot_dir.len() > u32::MAX` in serialization.

## 2025-05-24 - Schema Visualizer Injection
**Threat:**
`SchemaVisualizer::to_dot` blindly trusted user input (relvar names, attribute names) when constructing Graphviz DOT output. This allowed:
1. DOT Injection: An attacker could break out of the node identifier context using quotes (`"`) and inject arbitrary DOT commands (e.g., adding edges).
2. HTML Injection: An attacker could break out of the HTML-like label context using HTML tags (e.g., `</td>`) and corrupt the table visualization.

**Defense:**
1. Implemented `escape_dot_id` to strictly quote all DOT identifiers and escape internal quotes.
2. Implemented `escape_html` to sanitize strings used in HTML-like labels (replacing `<, >, &, ", '` with entities).
3. Implemented `escape_dot_string_content` for string literals in labels.
4. Updated `to_dot` to use these helpers for all user-controlled data.

## 2026-02-04 - HeapFile Header Corruption and DoS
**Threat:**
1. `HeapFile::try_insert_into_page` and related methods underestimated the page header size by manually calculating it (`size_of::<u32> + N * size`) instead of using `bincode`'s actual serialization size. `bincode` adds overhead (tags, varint lengths) not accounted for. This caused the header to grow into the tuple data area, leading to silent data corruption (overlap).
2. The discrepancy between `check_tuple_size_limit` (correctly using bincode) and `try_insert_into_page` (underestimating) could hypothetically lead to infinite loops if `check` passed but `insert` failed with `PageFull` repeatedly on new pages (though in this specific case it caused corruption instead of `PageFull`).

**Defense:**
Modified insertion logic to clone the page structure, insert a dummy entry into the target slot, and use `bincode::serialized_size` to calculate the exact header requirements before committing the write.
