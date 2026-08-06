Title: 🔒 Warden: [security fix] upgrade vulnerable dependencies

🦠 Threat: RUSTSEC-2026-0204 (crossbeam-epoch) and RUSTSEC-2026-0190 (anyhow). Invalid pointer dereference and unsound downcast respectively.
🛡️ Defense: Upgraded crossbeam-epoch to 0.9.20 and anyhow to 1.0.104
💥 Severity: Critical
🧪 Verification: ran cargo audit to confirm no vulnerable dependencies
