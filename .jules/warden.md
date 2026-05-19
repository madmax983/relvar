## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2026-04-25 - [Fix image DoS with i128 span checks]
**Threat:** Integer overflow Denial of Service (DoS) vulnerability in `save` function of `relvar/src/experimental/image.rs`. Bounding box math using `saturating_sub` on `i64` could cap out at `i64::MAX`, allowing spans exceeding `i64::MAX` to evade the `10_000_000` pixel limit through truncation when cast to `usize`. This leads to silent logic bypasses and out-of-bounds panics when manipulating pixel buffers.
**Defense:** Replaced `saturating_sub` math with safe `i128` arithmetic to accurately calculate `span_x` and `span_y`. Validated total capacity using `checked_mul` over `span_x` and `span_y` before converting to `usize`, ensuring any span exceeding limits returns an empty relation safely rather than triggering out-of-bounds accesses or allocating excessively.
