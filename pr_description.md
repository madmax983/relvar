Title: 🔒 Warden: [security fix]

🦠 Threat: Invalid pointer dereference in `crossbeam-epoch` (RUSTSEC-2026-0204) and unsoundness in `anyhow` (RUSTSEC-2026-0190) dependencies.
🛡️ Defense: Bumped the crates via `cargo update` to safe versions (`crossbeam-epoch` >= 0.9.20 and `anyhow` v1.0.104).
💥 Severity: High - Undefined behavior or other security issues.
🧪 Verification: Ran `cargo audit` successfully showing 0 vulnerabilities.
