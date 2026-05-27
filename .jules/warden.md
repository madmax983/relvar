## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2024-05-18 - RwLock Poisoning DoS
**Threat:** Calling `.unwrap()` on `RwLock` operations (like `read()` or `write()`) risks a system-wide panic cascade (Denial of Service) if a thread panics while holding the lock, poisoning it.
**Defense:** Replaced all `.unwrap()` calls on `RwLock` results in `PersistentEngine` (`relvar-storage/src/persistent_engine/mod.rs`) with safe error handling using `.map_err(|_| StorageError::Other("Lock poisoned".to_string()))?` for functions returning `Result` and `.unwrap_or_else(|e| e.into_inner())` for pure read operations.
