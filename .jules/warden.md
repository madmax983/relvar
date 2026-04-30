## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2026-04-25 - [Fix unwrap panics on storage manager locks in persistent engine]
**Threat:** Calling `.unwrap()` on `RwLock` operations around the storage manager introduces a potential panic point if the lock becomes poisoned (e.g. if another thread panics while holding the lock). This could lead to a cascading failure causing an unexpected Denial of Service.
**Defense:** Replaced all `self.storage_manager.read().unwrap()` and `self.storage_manager.write().unwrap()` calls in `PersistentEngine` with `.unwrap_or_else(|e| e.into_inner())` to safely recover the lock guard and prevent unexpected panics on poisoned locks.
