🔒 Warden: [security fix]

🦠 Threat: RUSTSEC-2026-0190 in `anyhow` v1.0.102 (Unsoundness in `Error::downcast_mut()`) AND a Denial of Service via unbounded memory allocation during image saving/loading (`relvar/src/experimental/image.rs`) due to implicit integer truncation on dimension multiplication. AND a DoS via unbounded CSV iteration.
🛡️ Defense: Upgraded `anyhow` to v1.0.103 and fortified `relvar/src/experimental/image.rs` using `abs_diff`, `try_from`, and `saturating_mul` before bounds checking the 10M pixel threshold. Restricted the CSV row loop inside `relvar/src/tools/importer.rs` to 100,000 maximum entries to defend against endless loops. Fixed test imports. Reverted erroneous storage exports to fix compilation. Removed duplicative code inside `relvar/src/experimental/image.rs`.
💥 Severity: High (DoS / Memory Exhaustion / Memory Corruption).
🧪 Verification: `cargo audit` passes. `warden_exploit_image_dos.rs` and `warden_exploit_csv_dos.rs` pass successfully without panics.
