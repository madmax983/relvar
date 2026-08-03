Title: 🔒 Warden: Bump vulnerable dependencies (crossbeam-epoch, anyhow)

🦠 Threat: Vulnerable dependency `crossbeam-epoch` v0.9.18 contained an invalid pointer dereference (RUSTSEC-2026-0204). Vulnerable dependency `anyhow` v1.0.102 contained an unsoundness issue (RUSTSEC-2026-0190).
🛡️ Defense: Updated `crossbeam-epoch` to v0.9.20 and `anyhow` to v1.0.104.
💥 Severity: High - These issues could cause crashes or unsoundness in multi-threaded/concurrent settings, and error handling paths.
🧪 Verification: Scanned with `cargo audit` and verified that both issues have been resolved.
