## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2025-02-28 - Image Processing Apply Kernel Panics
**Threat:** Panic / DoS via unhandled integer overflow during convolution coordinate shifting and weight scaling calculations.
**Defense:** Replaced bare `+` and `*` operators with `saturating_add` and `saturating_mul` to clamp boundary inputs safely and avoid debug-panic or release-wrap.

## 2025-02-28 - Unsound Dependency (anyhow)
**Threat:** `anyhow` version `1.0.102` contained RUSTSEC-2026-0190: Unsoundness in `Error::downcast_mut()`.
**Defense:** Bumped `anyhow` version to `1.0.103` using `cargo update -p anyhow`.
