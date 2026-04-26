## 2026-04-22 - [Prevent DoS in JSON Exporter]
**Threat:** Memory exhaustion DoS when exporting large relations to JSON due to in-memory accumulation of all tuples and single large string allocation.
**Defense:** Changed `to_json` to accept `std::io::Write` sink and stream serialized JSON iteratively using `SerializeSeq`.

## 2026-04-23 - [Float Overflow in Aggregation]
**Threat:** Floating point overflow during SUM/AVG operations yielding Infinity, potentially leading to logic bugs.
**Defense:** Added a check to ensure the aggregated sum remains finite, returning an error on overflow.

## 2026-04-26 - [Prevent DoS in Exporter Tools]
**Threat:** Memory exhaustion DoS when exporting large relations to CSV or ASCII tables due to in-memory accumulation of all output into a single large String allocation.
**Defense:** Refactored `to_csv` and `to_ascii_table` in `exporter.rs` to accept a `std::io::Write` sink and stream the serialized output iteratively.
