# 🔒 Warden: [security fix] Update crossbeam-epoch

* 🦠 Threat: Invalid pointer dereference in `fmt::Pointer` impl for `Atomic` and `Shared` (RUSTSEC-2026-0204)
* 🛡️ Defense: Upgraded `crossbeam-epoch` to >=0.9.20.
* 💥 Severity: High - possible undefined behavior and DoS.
* 🧪 Verification: Ran `cargo audit` to confirm vulnerability is fixed.
