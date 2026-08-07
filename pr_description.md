🦠 Threat: Known vulnerabilities in dependencies (`crossbeam-epoch` RUSTSEC-2026-0204: Invalid pointer dereference, and `anyhow` RUSTSEC-2026-0190: Unsoundness in `Error::downcast_mut()`).
🛡️ Defense: Bumped versions using `cargo update -p crossbeam-epoch` and `cargo update -p anyhow` to secure versions.
💥 Severity: High - Potential memory unsafety and pointer dereferences.
🧪 Verification: `cargo audit` now passes cleanly.
