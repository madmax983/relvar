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

## 2026-02-05 - Bincode Allocation Bomb DoS
**Threat:**
`relvar-storage` uses `bincode` 1.3.3, which is unmaintained and vulnerable to allocation bomb attacks. `bincode::deserialize` reads length prefixes from the input and pre-allocates memory based on that length before reading the actual data. An attacker could craft a malicious page containing a tuple with a huge declared length (e.g., 1GB) but very little data, causing the server to exhaust memory (DoS) or panic when attempting to allocate.

**Defense:**
1.  Implemented `deserialize_bounded` helper function in `HeapFile`.
2.  Configured `bincode::options()` with `.with_limit(PAGE_SIZE)` to reject any allocation request exceeding the page size (4KB). Since no tuple or structure within a page can validly exceed the page size, this prevents unbounded allocation.
3.  Preserved legacy configuration (`LittleEndian`, `FixedIntEncoding`, `AllowTrailingBytes`) to maintain compatibility with existing disk format.

## 2026-02-06 - HeapFile Infinite Loop DoS on Update
**Threat:**
`HeapFile::update_tuple_versioned` could enter an infinite loop (DoS) when attempting to update a tuple whose size is extremely close to the page capacity.
The size check `check_versioned_tuple_size_limit` underestimated the required space by assuming `prev_version` is `None` (1 byte), whereas updates insert a version with `prev_version: Some(...)` (9 bytes).
This discrepancy caused the size check to pass, but the actual insertion to fail with `PageFull`. The `find_page_for_insertion` loop would then retry infinitely, creating new empty pages until disk exhaustion.

**Defense:**
Modified `check_versioned_tuple_size_limit` to accept a `has_prev_version` boolean flag.
Updated `insert_tuple_versioned` to pass `false` and `update_tuple_versioned` to pass `true`.
This ensures the size check accurately accounts for the 8-byte overhead of the `prev_version` pointer during updates.

## 2026-02-07 - Like Operator Memory Exhaustion DoS
**Threat:**
The `ConstraintExpression::Like` operator implementation used a dynamic programming table of size `O(N*M)` (where N is text length and M is pattern length).
An attacker could supply a pattern and text both of large size (e.g., 1MB), causing a quadratic memory allocation (1 trillion bytes = 1TB) which would crash the process (OOM DoS).
Even with moderate sizes (e.g., 50KB), concurrent requests could exhaust server memory.

**Defense:**
Refactored `matches_pattern` to use an optimized DP approach that only stores two rows (current and previous) instead of the full matrix.
This reduces space complexity from `O(N*M)` to `O(M)`, preventing memory exhaustion even with large inputs.
Added a regression test `relvar-core/tests/warden_exploit_like.rs` to verify the fix and prevent regression.

## 2026-02-08 - Like Operator Large Input DoS
**Threat:**
The `ConstraintExpression::Like` operator implementation previously collected the input text into a `Vec<char>` for pattern matching.
An attacker could supply a very large text string (e.g., 100MB), causing a linear memory allocation of 4 bytes per character (400MB total), potentially leading to memory exhaustion (DoS).

**Defense:**
Refactored `matches_pattern` to iterate over `text.chars()` directly instead of collecting into a vector.
This reduces space complexity from $O(N+M)$ to $O(M)$ (where $M$ is pattern length), eliminating the DoS vector for large text inputs.
Added verification test `relvar-core/tests/warden_exploit_like_memory.rs`.

## 2026-02-09 - JSON Importer OOM DoS
**Threat:**
`importer::from_json` used `serde_json::from_reader` which deserialized the entire JSON input into a DOM (`Value`) before processing. An attacker could supply a very large JSON array (e.g., 10GB), causing the server to exhaust memory (OOM DoS) trying to represent the entire structure in memory.

**Defense:**
1. Replaced `from_json` implementation with a streaming parser using `serde::de::DeserializeSeed` and `Visitor`.
2. The new implementation iterates over the JSON array element-by-element, processing and inserting each tuple individually.
3. This ensures memory usage is proportional to the size of a single tuple (plus the accumulating `Relation` result), preventing the "double memory usage" (DOM + Result) and allowing for future optimizations (e.g. streaming to disk).

## 2026-02-10 - Recursive Type Definition Stack Overflow
**Threat:**
`ScalarType` and `TupleType` definitions allow for infinite recursion in type definitions (e.g., a user-defined type wrapping itself, or a tuple containing a relation of itself).
An attacker could define a deeply nested type structure programmatically or via API, causing the application to crash with a stack overflow during type comparison, hashing, or dropping (DoS).
Since `ScalarType` variants are public and `serde` deserialization bypasses constructors, this vulnerability could also be exploited via malicious payloads if strict depth limits aren't enforced during deserialization or usage.

**Defense:**
1. Added `MAX_TYPE_DEPTH` constant (64) to `relvar-core/src/types/mod.rs`.
2. Implemented recursive `depth()` method for `ScalarType`, `TupleType`, and `RelationType`.
3. Added panic checks in `ScalarType::user_defined`, `TupleType::with_attribute`, and `RelationType::new` to enforce the depth limit during construction.
4. While this primarily protects programmatic construction, it establishes a clear limit that should be respected by all future parsers and deserializers.

## 2026-02-11 - ScalarType Deserialization Stack Overflow
**Threat:**
Although `ScalarType::user_defined` constructor enforced `MAX_TYPE_DEPTH`, `ScalarType`'s enum variants were public, and it relied on the default `#[derive(Deserialize)]`. This allowed an attacker to construct a deeply nested `ScalarType` (e.g., via a JSON payload) bypassing the constructor checks, leading to a stack overflow or DoS when the type was subsequently used (e.g., hashed or dropped). `serde_json`'s recursion limit provided partial protection but was not guaranteed for all formats.

**Defense:**
1. Introduced a private `ScalarTypeUnchecked` enum mirroring `ScalarType` to intercept raw deserialization.
2. Implemented `TryFrom<ScalarTypeUnchecked> for ScalarType`, which recursively rebuilds the type and enforces the `MAX_TYPE_DEPTH` (64) limit.
3. Updated `ScalarType` to use `#[serde(try_from = "ScalarTypeUnchecked")]`.
4. This ensures that *any* deserialization of `ScalarType` must pass the depth check, regardless of the underlying format or its limits.
5. Added verification test `relvar-core/tests/warden_exploit_serde_stack_overflow.rs`.

## 2026-02-12 - JSON Importer Deserialization Bomb
**Threat:**
`relvar::tools::importer::from_json` used `serde_json` to deserialize input into a `JsonValue` DOM before converting to `ScalarValue`. This allowed "deserialization bombs" where a single malicious string or array (e.g., 1GB string) would be fully allocated in memory as `JsonValue`, potentially causing OOM DoS.

**Defense:**
1. Refactored `importer.rs` to use streaming `serde::de::DeserializeSeed` and `Visitor` traits, eliminating `JsonValue` usage.
2. Enforced strict limits `MAX_STRING_LEN` (1MB) and `MAX_BYTES_LEN` (1MB) during streaming parsing.
3. Added `ImporterError::LimitExceeded` to report violations.
4. Added `relvar/tests/warden_json_import.rs` to verify limits and recursion safety.
