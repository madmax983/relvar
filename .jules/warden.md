## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.

## 2025-05-24 - Image Exporter DoS Fix
**Threat:** A potential Denial of Service (DoS) vulnerability existed in `relvar::experimental::image::save`. The function calculates image dimensions (`width` and `height`) by subtracting the minimum bounding coordinate from the maximum (`max_x.saturating_sub(min_x).saturating_add(1)` as `usize`). If the result of the `saturating_sub` combined with `saturating_add` exceeded `usize::MAX`, it could truncate or wrap depending on architecture. Even more problematic, the result bounds check (`width.saturating_mul(height) > 10_000_000`) happens *after* an initial `saturating_add`, which means `i64::MAX.saturating_sub(-100)` -> `i64::MAX`. `i64::MAX.saturating_add(1)` -> `i64::MAX`. Casting to `usize` is fine on 64-bit, but the logic relies on `saturating_add(1)`. On 64-bit `i64::MAX` as `usize` is ~9 quintillion. However, the calculation could silently wrap or fail to properly handle negative domains in a strict boundary check.

**Defense:** Rewrote the coordinate domain calculation logic to compute strict relative differences `diff_x` and `diff_y` between maximum and minimum extents using `saturating_sub`. Then checked and safely converted the bounded difference `diff_x.checked_add(1)` into `usize` to properly allocate image vectors without integer wrap-arounds or memory bloat vectors. Replaced `(x - min_x) as usize` pixel indexing with safe `usize::try_from(x.saturating_sub(min_x))` to ensure invalid pixel mappings are cleanly ignored rather than panicking or overflowing memory boundaries during writes. Added a strict bounds-testing exploit proofing maximum bounds tests.
