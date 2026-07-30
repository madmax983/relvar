Title: 🔒 Warden: [security fix]

🦠 Threat: A logic bug in `serialize_slotted_page_with_tuples` allowed for unbounded slice indices by using `offset + length` without ensuring it did not overflow bounds. In addition, an outdated version of `crossbeam-epoch` (0.9.18) was vulnerable to a pointer deref vulnerability (CVE) and `anyhow` had unsound downcast_mut (CVE).
🛡️ Defense: Added bounds checking during slotted page serialization via `offset.checked_add` and ensuring that `end_offset <= data.len()`. Also updated dependencies via `cargo update -p crossbeam-epoch` and `cargo update -p anyhow` to resolve CVE vulnerabilities.
💥 Severity: Critical - could panic server and execute arbitrary code.
🧪 Verification: Added test cases and run cargo audit and cargo test.
