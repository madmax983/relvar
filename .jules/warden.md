## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2024-05-27 - RwLock Poisoning DoS
**Threat:** Calling `.unwrap()` on `RwLock::write()` or `RwLock::read()` allows a single poisoned lock (e.g. from a thread panicking during a write) to cascade and panic all subsequent threads attempting to acquire the lock, causing a system-wide Denial of Service.
**Defense:** Replaced `.unwrap()` calls on `RwLock` operations in `relvar-storage/src/persistent_engine/mod.rs` with safe `.map_err()` propagating a `StorageError`, or `.unwrap_or_else(|e| e.into_inner())` for pure read operations.
