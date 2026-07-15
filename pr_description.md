🦠 Threat: Denial of Service (DoS) via integer overflow in image bounds calculation.
🛡️ Defense: Used `abs_diff` and safe `usize::try_from` with `saturating_add` to prevent overflow and unbound memory allocation. Upgraded `crossbeam-epoch` to fix RUSTSEC-2026-0204.
💥 Severity: Critical - could panic server on maliciously crafted tuple data.
🧪 Verification: Added `warden_exploit_image_dos.rs` fuzzer test case.
