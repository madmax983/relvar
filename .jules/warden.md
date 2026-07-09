## 2024-07-08 - [Unsafe Type Cast Fix]
**Threat:** Integer limits from coordinates can be used to cause an DoS panic due to saturating operations that result in large length limits overflowing on usize conversion causing panics. `as usize` operations may silently truncate limits or allocate invalid limits.
**Defense:** Replaced `as usize` casts with safe `usize::try_from` operations.

## 2024-07-08 - [Dependency Vulnerability Fix]
**Threat:** The `anyhow` crate version 1.0.102 has an unsoundness vulnerability in `Error::downcast_mut()` and `crossbeam-epoch` version 0.9.18 has an invalid pointer dereference vulnerability.
**Defense:** Upgraded `anyhow` to 1.0.103 and `crossbeam-epoch` to 0.9.20.
