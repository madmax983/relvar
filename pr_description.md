Title: '🔒 Warden: Fix RwLock poisoning in PersistentEngine'
🦠 Threat: Calling `.unwrap()` on `RwLock::write()` or `RwLock::read()` in `relvar-storage/src/persistent_engine/mod.rs` allows a single poisoned lock (e.g. from a thread panicking during a write) to cascade and panic all subsequent threads attempting to acquire the lock, causing a system-wide Denial of Service.
🛡️ Defense: Replaced `.unwrap()` calls on `RwLock` operations with safe `.map_err()` propagating a `StorageError`, or `.unwrap_or_else(|e| e.into_inner())` for pure read operations.
💥 Severity: Critical - could panic server and cause a system-wide Denial of Service.
🧪 Verification: Ran `cargo test`, `cargo check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all`. Verified successful test passes.
