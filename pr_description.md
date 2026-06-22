🔒 Warden: Fix integer overflow vulnerabilities

🦠 Threat: Several operations in `relvar-core` performed unchecked integer arithmetic (`count + 1`, `sum += value`), which could lead to integer overflow panics and potential Denial of Service (DoS) when processing high-cardinality datasets or extreme values.

🛡️ Defense: Hardened operations by replacing unchecked math with safe variants like `.saturating_add()` or `.checked_add()` to ensure safety without panics.

💥 Severity: Medium - Could cause query execution to panic and crash the database process when encountering boundary cases.

🧪 Verification: Ran `cargo clippy`, `cargo test`, and `cargo audit` to confirm the absence of vulnerabilities and the correctness of the safe math operations.
