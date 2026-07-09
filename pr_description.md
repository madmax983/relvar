🦠 Threat: Integer truncations or silent panics by using `as usize` rather than safe `usize::try_from` casting. There were also security issues in dependencies, specifically `anyhow` (unsoundness in downcast_mut, CVE-2026-0190) and `crossbeam-epoch` (invalid pointer dereference, CVE-2026-0204).
🛡️ Defense: Replaced unsafe `as usize` coercions with safe `usize::try_from()` mappings along with `abs_diff` calculations. Upgraded vulnerable crates via cargo update.
💥 Severity: Critical - could panic server and cause memory vulnerabilities.
🧪 Verification: Tested with `cargo audit` to confirm fixes, and `cargo test` to ensure stability.
