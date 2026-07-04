🔒 Warden: Fix RUSTSEC-2026-0190 and unsafe as usize casts

🦠 Threat: Unsoundness in `Error::downcast_mut()` in `anyhow` dependency (RUSTSEC-2026-0190) and Integer Overflow vulnerabilities allowing potential DoS or memory exhaustion due to blind `as usize` casts across codebase.
🛡️ Defense: Updated `anyhow` to a secure version. Refactored all `as usize` boundary casts to use `usize::try_from(...)` with proper error propagation (`HeapError` or saturating defaults), avoiding logic bugs from silent `.unwrap_or(0)` corruptions.
💥 Severity: High - Unsound behavior and potential panics or unbounded memory limits.
🧪 Verification: Ran `cargo audit` to confirm vulnerability resolution. Executed `cargo test`, `cargo clippy`, and `cargo fmt` to verify safe math replacement behavior and maintain compilation soundness.
