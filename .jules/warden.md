## 2026-03-10 - Unsafe unwrap calls removal
**Threat:**
Using `unwrap()` in critical functions like `from_le_bytes` combined with `try_into()` on slices can lead to panic if assumptions fail. The `Ord` implementation on `ScalarType` had `unwrap()` within mapping over attribute names. A panic within a basic trait implementation could be triggered intentionally, acting as a Denial of Service vector.
**Defense:**
Refactored `from_le_bytes` by using securely sized slice conversions without `.unwrap()` in `relvar-storage/src/storage/page.rs` and `relvar-storage/src/storage/heap.rs`. Replaced `.unwrap()` in `ScalarType::cmp` by directly iterating over `.attributes()` instead of `.attribute_names()`.

## 2026-03-10 - Unsafe unwrap calls removal
**Threat:**
Using `unwrap()` in critical functions like `from_le_bytes` combined with `try_into()` on slices can lead to panic if assumptions fail. The `Ord` implementation on `ScalarType` had `unwrap()` within mapping over attribute names. A panic within a basic trait implementation could be triggered intentionally, acting as a Denial of Service vector.
**Defense:**
Refactored `from_le_bytes` by using securely sized slice conversions without `.unwrap()` in `relvar-storage/src/storage/page.rs` and `relvar-storage/src/storage/heap.rs`. Replaced `.unwrap()` in `ScalarType::cmp` by directly iterating over `.attributes()` instead of `.attribute_names()`.
