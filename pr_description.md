🦠 Threat: The `save` function in `relvar/src/experimental/image.rs` subtracted two `i64` coordinates directly (`x - min_x`) and cast the result to `usize`. If `x` was `i64::MIN` and `min_x` was positive, this would underflow and cause a runtime panic. An attacker could craft a relation with extreme coordinate values to intentionally crash the database engine, resulting in a Denial of Service (DoS). Additionally, the raw buffer length multiplier could potentially overflow if unchecked.

🛡️ Defense: Upgraded the coordinate span calculation to use `i128` to safely handle the full distance between `i64::MIN` and `i64::MAX`. Replaced unsafe subtractions with `saturating_sub` and `saturating_add`. Implemented robust buffer size checks using chained `checked_mul` and bounds checking on coordinate assignment to ensure no panics occur even with malformed tuples.

💥 Severity: High - Unauthenticated crash/DoS vector if the database is configured to ingest arbitrary coordinate relations.

🧪 Verification: Added `test_save_dos_panic_prevention` (in a separate test file) to fuzz the engine with `i64::MAX` and `i64::MIN` coordinates, demonstrating that the engine safely ignores out-of-bounds pixels rather than panicking. Ran `cargo test`, `cargo audit`, and `cargo clippy`.
