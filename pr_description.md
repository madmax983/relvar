🦠 Threat: `crossbeam-epoch` had a vulnerability with invalid pointer dereferences (RUSTSEC-2026-0204), and `anyhow` had unsoundness in `Error::downcast_mut()` (RUSTSEC-2026-0190).
🛡️ Defense: Updated `crossbeam-epoch` to `v0.9.20` and `anyhow` to `v1.0.104`.
💥 Severity: Critical - could lead to undefined behavior and invalid pointer dereferences.
🧪 Verification: `cargo audit` runs successfully with no vulnerabilities found.
