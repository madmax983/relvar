## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2024-05-18 - Math Overflows and Integer Wrapping Security Review
**Threat:** Mathematical operations such as integer additions during iteration and sizing computations (`offset`, `page_id`, `slot_count`, `total_capacity`) were vulnerable to potential overflow or wrapping behavior if large buffers or extreme states were reached, leading to out-of-bounds access, logic corruption, or panics.
**Defense:** Replaced bare `+=` arithmetic with explicit `checked_add().expect(...)` ensuring that any capacity bounds are rigorously checked and program fails safely instead of continuing with corrupted logic, mitigating Denial of Service (DoS) and out-of-bounds risks.
