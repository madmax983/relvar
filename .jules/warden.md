## 2024-05-23 - HeapFile Error Handling and KeyConstraints Panic
**Threat:**
1. `HeapFile` operations (`insert_tuple`, `scan`) were masking I/O errors (e.g., permission denied, hardware failure) by treating them as EOF/New Page.
2. `KeyConstraints::would_violate` could panic if passed a tuple missing a key attribute.

**Defense:**
1. Modified `HeapFile::try_insert_into_page` and `scan` to propagate errors from `read_page` using `?`. Note: `read_page` handles EOF by returning `Ok(empty_page)`, so propagating `Err` correctly surfaces *real* I/O errors without breaking page growth.
2. Modified `KeyConstraints::extract_key_value` to return `Result` and added `KeyConstraintError::TupleMissingAttribute`. Updated call sites to propagate this error.

## 2025-05-27 - Storage Integer Overflow & DoS Hardening
**Threat:**
1. Integer Overflow in `PageFile::read_page`: Maliciously crafted page files could set `data_len` to `usize::MAX - 7`, causing `data_len + 8` to wrap to 0. This bypasses length checks (`0 <= buffer.len()`) and causes a panic when slicing `buffer[8..0]` (DoS).
2. Integer Overflow in `HeapFile` operations (`scan`, `insert`, etc.): Slot offsets and lengths are read from disk as `u32`. Calculating `start + length` could overflow or wrap, leading to out-of-bounds slice access and panics.

**Defense:**
1. Hardened `PageFile::read_page` to verify `data_len <= buffer.len().saturating_sub(8)` before any addition or slicing.
2. Refactored `HeapFile` to use a helper method `get_slot_slice` which uses `checked_add` for all slot offset calculations and strictly validates ranges against the page buffer size, returning `HeapError::Serialization` instead of panicking.
