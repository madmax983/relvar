🦠 Threat: `crossbeam-epoch` (v0.9.18) contains an invalid pointer dereference vulnerability (RUSTSEC-2026-0204) and `anyhow` (v1.0.102) contains an unsoundness vulnerability (RUSTSEC-2026-0190).
🛡️ Defense: Bumped `crossbeam-epoch` to `v0.9.20` and `anyhow` to `v1.0.104` to resolve the vulnerabilities.
💥 Severity: High - Invalid pointer dereference and unsoundness could lead to Undefined Behavior (UB) and potential exploits.
🧪 Verification: `cargo audit` passes cleanly.
