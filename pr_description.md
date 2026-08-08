🦠 Threat: Vulnerable dependencies identified by cargo audit. `crossbeam-epoch` v0.9.18 contained an invalid pointer dereference (RUSTSEC-2026-0204), and `anyhow` v1.0.102 had unsoundness in `Error::downcast_mut()` (RUSTSEC-2026-0190).
🛡️ Defense: Updated `crossbeam-epoch` to v0.9.20 and `anyhow` to v1.0.104.
💥 Severity: Critical - could lead to undefined behavior, panics, or arbitrary code execution.
🧪 Verification: Ran `cargo audit` to confirm clean dependency tree and `cargo test` to ensure functionality remains sound.
