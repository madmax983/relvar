## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.
## 2025-02-15 - Serde Recursion Guard for ASTs
**Threat:** The `Query` and `ConstraintExpression` enums are recursive and use `postcard` for deserialization without a depth limit, which enables DoS attacks via stack overflows.
**Defense:** Created intermediate unchecked enums (`QueryUnchecked` and `ConstraintExpressionUnchecked`) and applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to their `Box`ed fields. Mapped them to the actual ASTs via `#[serde(try_from = "QueryUnchecked")]` and `#[serde(try_from = "ConstraintExpressionUnchecked")]`.
