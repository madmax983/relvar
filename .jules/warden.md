## 2026-04-10 - Heap Storage Arithmetic Overflow DoS
**Threat:** The `HeapFile` module (`relvar-storage/src/storage/heap.rs`) contained potential integer overflows during memory calculation for serialization, such as when adding tuple size + header size. A maliciously crafted data structure or an unexpectedly large entry count could overflow these raw additions (`header_size + tuple_data_len`, `header_size + total_tuple_data_size`), circumventing size limits (`USABLE_PAGE_SIZE`) and potentially causing out-of-bounds writes into buffers or Panics within the node.
**Defense:** Replaced bare `+` arithmetic operators with `.checked_add(...)` combined with `.ok_or_else(|| ...)` for size constraint checks, accurately detecting and mitigating overflow before bounds are enforced, resulting in a safe `HeapError::Serialization` instead of UB or panics.

## 2026-04-10 - Image Dimensions DoS
**Threat:** The image module (`relvar/src/experimental/image.rs`) was vulnerable to an Out-of-Memory (OOM) and CPU Exhaustion DoS attack. The `save` function computed dimensions using simple arithmetic `(max_x - min_x + 1)` which could underflow or overflow, and then allocated a flat buffer `vec![0u8; width * height * 3]`. An attacker could supply two tuples with extremely distant coordinates (e.g. `(0,0)` and `(1_000_000, 1_000_000)`) causing the memory allocator to attempt reserving terabytes of RAM. The `load` function also accepted arbitrary `width` and `height` parameters resulting in extreme loop evaluations.
**Defense:** Both `save` and `load` were updated to strictly cap pixel counts. `width` and `height` calculations in `save` were updated to use safe arithmetic (`max_x.saturating_sub(min_x).saturating_add(1)`). Both functions now evaluate `width.saturating_mul(height) > 10_000_000`, immediately and safely returning empty results if the pixel threshold is breached.
## 2026-04-11 - Unsoundness in rand
**Threat:** The `rand` crate versions 0.8.5 and 0.9.2 contain a vulnerability (RUSTSEC-2026-0097) where they are unsound when using a custom logger with `rand::rng()`.
**Defense:** Updated the `rand` dependency to 0.10.1 and `proptest` to 1.11.0 to pull in a secure version of `rand`. Updated `relvar/src/experimental/mock.rs` to match the new `rand` API (`random()`, `random_range()`, `RngExt`, `distr::Alphanumeric`, and `StdRng::from_rng(&mut rand::rng())`).

## 2026-04-12 - LimitExceeded Silent Truncation
**Threat:** The `importer.rs` json import and `catalog.rs` load functions were using `serde_json::from_reader(capped_reader)`. While the reader properly capped bounds (10MB limit), if the reader exhausted its cap during stream parsing, `serde_json` could silently truncate valid partial streams or emit a generic JSON parse error instead of enforcing memory safety visibility, leaving the application vulnerable to partial unvalidated data loads or mimicking data integrity on 0-metadata length unbounded device files.
**Defense:** Added explicit trailing validation of `capped_reader.limit() == 0` strictly post-deserialization. If the cap hits 0, the functions explicitly map the result to `LimitExceeded` / `CatalogError` regardless of prior silent or EOF conditions, assuring full data integrity validation and preventing unbounded stream spoofing.

## 2026-04-12 - CSV Injection Prevention
**Threat:** The CSV exporter (`relvar/src/tools/exporter.rs`) blindly exported strings without validation. An attacker could craft a tuple with a `ScalarValue::String` starting with `=`, `+`, `-`, `@`, `\t`, `\r`, or `\n` (e.g. `=cmd|' /C calc'!A0`). When the resulting CSV is opened in spreadsheet software like Excel, the software may interpret this as a formula or DDE execution command, leading to Arbitrary Code Execution on the client's machine (Formula Injection / CSV Injection).
**Defense:** Inside `format_scalar_csv`, updated `ScalarValue::String` serialization to detect if the first character matches any of the formula execution trigger characters. If a match is found, a single quote (`'`) is safely prepended to the string before standard CSV quoting. This forces spreadsheet engines to interpret the payload safely as a literal string instead of an executable formula.

## 2024-05-18 - [Add bounds checks]
**Threat:** Buffer overflows via int arithmetic
**Defense:** Replace arithmetic operations with checked variants
