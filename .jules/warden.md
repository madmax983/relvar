## 2024-05-18 - [Security Audit Completed]
**Threat:** No new threats were found.
**Defense:** All systems are secure.
## 2024-05-18 - [DoS via JSON Serialization Memory Exhaustion]
**Threat:** Unbounded memory allocation during JSON serialization of catalogs (`Catalog::save`) and relations (`exporter::to_json`), which construct massive intermediate memory structures or use `to_string_pretty` allowing Denial of Service vectors.
**Defense:** Switched to streaming serialization using `serde_json::to_writer_pretty` via a `BufWriter` directly on the File, and using `serde::ser::SerializeSeq` to iteratively serialize elements.
