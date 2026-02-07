## 2025-05-24 - Extract ConstraintManager
**Tangle:** `Database` struct in `relvar-core/src/database.rs` was becoming a "God Struct", mixing database orchestration with constraint enforcement logic. The file was nearly 2000 lines long.
**Blueprint:** Extracted `ConstraintManager` and its error types into a dedicated `relvar-core/src/constraints/manager.rs` module. This improves cohesion by grouping constraint logic with constraint definitions and reduces coupling in the main database module.

## 2025-05-24 - Unify Database Error Handling
**Tangle:** `DatabaseError` duplicated `ConstraintManagerError` variants (violating DRY) and required manual synchronization.
**Blueprint:** Nested `ConstraintManagerError` within `DatabaseError` via a `Constraint(#[from] ConstraintManagerError)` variant. This enforces hierarchy and removes code duplication.

## 2025-05-25 - Refactor PersistentEngine Structure
**Tangle:** `PersistentEngine` in `relvar-storage/src/persistent_engine.rs` was a "Blob" at the root of the source directory, mixing orchestration with implementation details.
**Blueprint:** Moved `PersistentEngine` to its own module `relvar-storage/src/engine/mod.rs` to enforce better module boundaries and prepare for further decomposition.

## 2025-05-25 - Fix ScalarType Feature Envy
**Tangle:** `ScalarType` in `relvar-core` contained complex logic to compare `RelationType`s, violating encapsulation ("Feature Envy").
**Blueprint:** Implemented `Ord` for `RelationType` and `TupleType` and refactored `ScalarType` to delegate comparison, enforcing clean abstractions.
