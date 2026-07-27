Title: 🔒 Warden: [security fix] PersistentEngine DoS lock poisoning & Dependency Updates

🦠 Threat:
1. The `PersistentEngine` had multiple `.unwrap()` calls on acquiring `RwLock`s for `storage_manager`. If a thread panicked while holding the lock, the lock became poisoned. Any subsequent thread attempting to acquire the lock would panic at `.unwrap()`, leading to a denial-of-service cascade.
2. The `Cargo.lock` file contained vulnerable versions of `crossbeam-epoch` (Invalid pointer dereference, RUSTSEC-2026-0204) and `anyhow` (Unsoundness in `Error::downcast_mut()`, RUSTSEC-2026-0190).

🛡️ Defense:
1. Replaced `.unwrap()` calls in `relvar-storage/src/persistent_engine/mod.rs` with `.map_err(|_| StorageError::Other("StorageManager lock poisoned".to_string()))?` to gracefully propagate errors rather than panicking.
2. Updated `crossbeam-epoch` to `0.9.20` and `anyhow` to `1.0.104` via `cargo update` to patch known security advisories.

💥 Severity: High - A single thread crash could take down the entire `PersistentEngine` instance due to poisoned lock propagation. Dependency vulnerabilities could lead to memory unsafety and pointer dereferences.

🧪 Verification: Ran `cargo audit` to confirm dependencies are secure, and ran `cargo clippy`, `cargo test`, and `cargo fmt` to verify codebase soundness and logic.
