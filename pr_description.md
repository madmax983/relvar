# 🔒 Warden: Fix DoS from RwLock Poisoning & Update Dependencies

🦠 Threat:
- Application panic due to `.unwrap()` calls on `storage_manager.read()` and `storage_manager.write()` in `PersistentEngine`. If a thread panics while holding the lock, it poisons the `RwLock`, causing subsequent threads to panic, resulting in a denial-of-service (DoS) cascade.
- Outdated dependencies (`anyhow` and `crossbeam-epoch`) contained vulnerabilities (`RUSTSEC-2026-0190` and `RUSTSEC-2026-0204`).

🛡️ Defense:
- Replaced `.unwrap()` with safe error handling (`.map_err(...)`) on lock acquisition in `PersistentEngine`, propagating a `StorageError::Other` on lock poisoning.
- Updated `crossbeam-epoch` to `v0.9.20` and `anyhow` to `v1.0.104` to eliminate these vulnerabilities.

💥 Severity: High - DoS cascade and potential UB/crashes from dependency vulnerabilities.

🧪 Verification: Ran `cargo audit` to confirm dependencies are secure, and ran tests to confirm normal operations work as expected without panicking.
