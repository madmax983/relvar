## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.

## 2026-07-05 - [Integer Overflow in Image Loading/Saving]
**Threat:** A Denial of Service (DoS) due to unsafe bounds calculation in `image::save` and `image::load` where negative bounds interacting with `usize` casting and `saturating_add/sub` could cause integer overflows, panic, or attempt to allocate an immense amount of memory.
**Defense:** Replaced naive arithmetic and casting with `abs_diff` and explicit fallible integer conversions using `usize::try_from` safely defaulting to `usize::MAX` on error, ensuring graceful fallback and size limits checking.

## 2026-07-05 - [Vulnerable Dependency in anyhow]
**Threat:** The `anyhow` crate version `1.0.102` had a known vulnerability (RUSTSEC-2026-0190) involving unsoundness in `Error::downcast_mut()`.
**Defense:** Bumped `anyhow` dependency to `1.0.103` using `cargo update -p anyhow` to resolve the vulnerability.
