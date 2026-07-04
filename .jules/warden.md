## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.

## 2024-06-25 - Fix RUSTSEC-2026-0190 in anyhow and Integer Overflow using as usize
**Threat:** Unsoundness in `Error::downcast_mut()` in `anyhow` dependency and Potential Denial of Service (DoS) / Logic bugs / Memory exhaustion due to integer overflow resulting from unsafe `as usize` casts across the codebase.
**Defense:** Updated the `anyhow` crate to secure version via `cargo update -p anyhow`. Replaced `as usize` casts with safe `usize::try_from(...).map_err(...)` propagating safe Error types, and implemented saturating math where error propagation isn't feasible (e.g. image processing boundaries).
