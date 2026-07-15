🦠 Threat: Memory allocation exhaustion DoS in `relvar/src/experimental/image.rs` `save()` function, triggered by integer truncation and overflow during dimension calculation where `(max_x.saturating_sub(min_x).saturating_add(1)) as usize` failed when `min_x` and `max_x` spanned large intervals.
🛡️ Defense: Refactored dimension calculation to use `abs_diff` and safe conversion `usize::try_from()`, bounding size limits properly before allocating the image buffer. Updated dependencies with known CVEs.
💥 Severity: High - A maliciously crafted relation could DoS the server via unbounded memory allocation on image export.
🧪 Verification: Added `warden_exploit_image_dos.rs` and ran `cargo test`, `cargo audit`.
