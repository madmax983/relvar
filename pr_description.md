🔒 Warden: [security fix] anyhow RUSTSEC-2026-0190

🦠 Threat: Unsoundness in `Error::downcast_mut()` in `anyhow` crate version 1.0.102 (RUSTSEC-2026-0190).
🛡️ Defense: Updated `anyhow` crate to 1.0.103 in Cargo.lock.
💥 Severity: High - Unsoundness in a widely used error handling library could lead to undefined behavior or memory safety issues.
🧪 Verification: Confirmed with `cargo audit` that no vulnerabilities remain.
