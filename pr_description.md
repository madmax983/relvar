🦠 Threat: `cargo audit` identified two vulnerabilities in dependencies: `crossbeam-epoch` (RUSTSEC-2026-0204: Invalid pointer dereference when printing `Atomic` and `Shared` with `fmt::Pointer`) and `anyhow` (RUSTSEC-2026-0190: Unsoundness in `Error::downcast_mut()`).
🛡️ Defense: Upgraded `crossbeam-epoch` from 0.9.18 to 0.9.20, and `anyhow` from 1.0.102 to 1.0.104.
💥 Severity: Critical - could allow undefined behavior, invalid pointer dereferences, or memory corruption.
🧪 Verification: Ran `cargo audit` and verified that no active vulnerabilities remain in `Cargo.lock`.
