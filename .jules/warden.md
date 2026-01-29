## HeapFile Error Handling and KeyConstraints Panic
**Threat:**
1. `HeapFile` operations (`insert_tuple`, `scan`) were masking I/O errors (e.g., permission denied, hardware failure) by treating them as EOF/New Page.
2. `KeyConstraints::would_violate` could panic if passed a tuple missing a key attribute.

**Defense:**
1. Modified `HeapFile::try_insert_into_page` and `scan` to propagate errors from `read_page` using `?`. Note: `read_page` handles EOF by returning `Ok(empty_page)`, so propagating `Err` correctly surfaces *real* I/O errors without breaking page growth.
2. Modified `KeyConstraints::extract_key_value` to return `Result` and added `KeyConstraintError::TupleMissingAttribute`. Updated call sites to propagate this error.

## Database Update Constraint Bypass
**Threat:** `Database::update` bypassed Type, Check, and Foreign Key constraints, allowing invalid data to be persisted and referential integrity to be violated.
**Defense:** Refactored constraint validation logic into helper methods and enforced them in `Database::update`.
