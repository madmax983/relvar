Title: 🔒 Warden: [security fix] Update vulnerable dependencies crossbeam-epoch and anyhow

🦠 Threat: Invalid pointer dereference in `crossbeam-epoch` (RUSTSEC-2026-0204) and unsoundness in `anyhow` (RUSTSEC-2026-0190).
🛡️ Defense: Bumbped `crossbeam-epoch` to `0.9.20` and `anyhow` to `1.0.104`.
💥 Severity: Critical - could lead to undefined behavior, memory unsafety, and potential exploitation.
🧪 Verification: Ran `cargo audit` to ensure no active vulnerabilities remain.
