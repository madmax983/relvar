🔒 Warden: Fix image process panics & anyhow unsoundness

🦠 Threat:
1. Dependency `anyhow` version `1.0.102` contained RUSTSEC-2026-0190: Unsoundness in `Error::downcast_mut()`.
2. Panic / DoS via unhandled integer overflow during convolution coordinate shifting and weight scaling calculations in `relvar::experimental::image::apply_kernel`.

🛡️ Defense:
1. Bumped `anyhow` version to `1.0.103`.
2. Replaced bare `+` and `*` operators with `saturating_add` and `saturating_mul` in `relvar::experimental::image::apply_kernel` to safely clamp inputs.

💥 Severity: Moderate - could cause server crash (DoS) on malicious image convolution parameters or crafted relation payloads. Unsoundness in anyhow might lead to undefined behavior on error downcasting.

🧪 Verification: Added fuzzing-style test cases verifying `apply_kernel` with max integers correctly avoids overflow panics.
