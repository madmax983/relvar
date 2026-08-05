Title: 🔒 Warden: Bump vulnerable dependencies

🦠 Threat: Cargo.lock contained vulnerable dependencies (`crossbeam-epoch` and `anyhow`).
🛡️ Defense: Updated `crossbeam-epoch` to `v0.9.20` and `anyhow` to `v1.0.104` which fixes the vulnerabilities.
💥 Severity: High - invalid pointer dereference and unsoundness could lead to crashes or UB.
🧪 Verification: Verified via `cargo audit`.
