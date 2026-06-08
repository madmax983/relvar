## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.

## 2024-06-08 - [Unbounded Recursion DoS in ASTs]
**Threat:** Deeply nested JSON/Postcard payloads for `Query` and `ConstraintExpression` ASTs can trigger a stack overflow DoS during `serde` deserialization due to unbounded recursive `Box` fields.
**Defense:** Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to the recursive `Box` fields to securely enforce a maximum depth limit.
