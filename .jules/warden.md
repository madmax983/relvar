## 2026-10-24 - DoS and Memory Vulnerability Fixes via Safe Casts and Audits
**Threat:** Integer overflows leading to silent truncation via `as usize` casts in page length parsing, tuple slot indexing, and image dimension calculation. Unpatched `anyhow` and `crossbeam-epoch` crates contained known memory/unsoundness CVEs.
**Defense:** Enforced `usize::try_from` with appropriate safe fallbacks (`usize::MAX`) or explicit error propagation (`PageError`, `HeapError`) for unconstrained `as usize` casts. Upgraded dependencies via `cargo update` to neutralize known advisories.
