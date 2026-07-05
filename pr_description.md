🔒 Warden: [security fix]

🦠 Threat: A Denial of Service (DoS) vulnerability in `image::save` and `image::load` caused by unbounded arithmetic allocations and integer overflows when handling negative image bounds coupled with `usize` casting. This could result in out of bounds memory allocations. Furthermore, the `anyhow` crate version `1.0.102` had a known vulnerability (RUSTSEC-2026-0190) involving unsoundness in `Error::downcast_mut()`.
🛡️ Defense: Refactored the bound calculation arithmetic in `image::save` and `image::load` using `abs_diff`, safe size checks with `saturating_add`, and explicit fallback utilizing `usize::try_from(...)` to bound sizes reasonably up to max limits and prevent overflow. Bumped `anyhow` dependency to `1.0.103` using `cargo update -p anyhow`.
💥 Severity: Critical - allows crashing or exhausting memory (DoS).
🧪 Verification: Fixed bounds calculation tests and confirmed via `cargo audit` that the vulnerable dependency is resolved.
