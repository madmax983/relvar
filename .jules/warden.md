## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.

## 2024-06-16 - [Deserialization DoS on ASTs]
**Threat:** A deserialization vulnerability existed in the `Query` and `ConstraintExpression` enums. Because they contain deeply recursive fields (`Box<Query>` and `Box<ConstraintExpression>`) and were using the default `#[derive(Deserialize)]`, an attacker could craft a deeply nested JSON payload that would trigger a stack overflow during `serde_json::from_str`, resulting in a Denial of Service.
**Defense:** Replaced the direct `#[derive(Deserialize)]` on `Query` and `ConstraintExpression` with `#[serde(try_from = "...Unchecked")]`. The unchecked enums mirror the original definitions but apply `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to all recursive `Box` fields. This limits the recursion depth (via `RecursionGuard`) and returns a safe error instead of crashing the process.
