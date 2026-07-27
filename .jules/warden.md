## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2024-06-13 - [Denial of Service via Unbounded Recursion]
**Threat:** Stack overflow Denial of Service (DoS) vulnerability triggered by malicious actors submitting deeply nested `Query` or `ConstraintExpression` AST structures. `postcard` has no default recursion depth limits, meaning deserialization of deep inputs directly overflows the call stack, crashing the server process.
**Defense:** Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to the recursive `Box` fields in both `Query` and `ConstraintExpression` enums.

## 2026-05-24 - [RwLock Poisoning DoS in PersistentEngine]
**Threat:** Application panic due to `.unwrap()` calls on `storage_manager.read()` and `storage_manager.write()`. If a thread panics while holding the lock, it poisons the `RwLock`, causing subsequent threads to panic, resulting in a denial-of-service (DoS) cascade.
**Defense:** Replaced `.unwrap()` with safe error handling on lock acquisition in `PersistentEngine`. Used `.map_err()` to propagate `StorageError::Other` for operations returning a `Result`, and used `.unwrap_or_else(|e| e.into_inner())` for read-only methods returning non-Results, safely extracting the guard without propagating a panic.

## 2026-07-27 - [PersistentEngine RwLock Poisoning DoS & crossbeam-epoch/anyhow Vulns]
**Threat:** Application panic due to `.unwrap()` calls on `storage_manager.read()` and `storage_manager.write()`. If a thread panics while holding the lock, it poisons the `RwLock`, causing subsequent threads to panic, resulting in a denial-of-service (DoS) cascade. Dependencies `crossbeam-epoch` and `anyhow` also had known vulnerabilities.
**Defense:** Replaced `.unwrap()` with safe error handling on lock acquisition in `PersistentEngine`. Used `.map_err()` to propagate `StorageError::Other` for operations returning a `Result`. Upgraded vulnerable dependencies via `cargo update`.
