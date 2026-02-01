# Atlas's Architectural Journal

## 2024-05-22 - Splitting The Database Blob
**Tangle:** `relvar-core/src/database.rs` was approaching 1000 lines, mixing high-level database orchestration with low-level constraint validation logic ("The Bloat" and "Sprawl"). `ConstraintManager` was an internal detail leaking into the public API of the file, though not re-exported.
**Blueprint:**
1. Extracted `ConstraintManager` to `database/constraint_manager.rs` and restricted visibility to `pub(crate)` (Encapsulation).
2. Extracted `DatabaseError` to `database/error.rs` (Standardization).
3. Converted `database.rs` to `database/mod.rs` to act as the facade for the module.
4. Reduced coupling: `ConstraintManager` now focuses solely on validation, while `Database` handles orchestration.
