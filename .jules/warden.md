## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-24 - [Unsafe unwrap in StorageManager]
**Threat:** Potential panic and crash if `get_or_open_heap_file` fails to retrieve or map a file correctly, resulting in an unhandled `.unwrap()` failure.
**Defense:** Replaced `.unwrap()` with `HashMap::entry` to ensure safe, graceful error propagation rather than crashing the system without unreachable branches.

## 2024-05-18 - Integer Underflow Denial of Service in Image Processing
**Threat:** The `save` function in `relvar/src/experimental/image.rs` subtracted two `i64` coordinates directly (`x - min_x`) and cast the result to `usize`. If `x` was `i64::MIN` and `min_x` was positive, this would underflow, causing a runtime panic. An attacker could craft a relation with extreme coordinate values to intentionally crash the database engine, resulting in a Denial of Service (DoS).
**Defense:** Upgraded coordinate span logic to use `i128` to safely encompass the full `i64` distance without overflow. Applied `saturating_sub` and `saturating_add` for intermediate coordinate math. Additionally, fortified buffer allocation using `checked_mul` and `checked_add` chains rather than relying on direct multiplication, returning an empty buffer if limits or bounds are violated.
