## 2025-05-24 - Extract ConstraintManager
**Tangle:** `Database` struct in `relvar-core/src/database.rs` was becoming a "God Struct", mixing database orchestration with constraint enforcement logic. The file was nearly 2000 lines long.
**Blueprint:** Extracted `ConstraintManager` and its error types into a dedicated `relvar-core/src/constraints/manager.rs` module. This improves cohesion by grouping constraint logic with constraint definitions and reduces coupling in the main database module.

## 2025-05-24 - Unify Database Error Handling
**Tangle:** `DatabaseError` duplicated `ConstraintManagerError` variants (violating DRY) and required manual synchronization.
**Blueprint:** Nested `ConstraintManagerError` within `DatabaseError` via a `Constraint(#[from] ConstraintManagerError)` variant. This enforces hierarchy and removes code duplication.

## 2025-05-24 - Trusted Internal Algebra
**Tangle:** O(N) tuple validation overhead in core algebra operators (`project`, `join`, `rename`, `union`) due to public API constraints.
**Blueprint:** Introduced `pub(crate)` trusted constructors `Tuple::new_unchecked` and `Relation::from_tuples_unchecked`. This separates the "Public Safety" boundary from the "Internal Performance" core, allowing algebra operators to bypass redundant checks when data integrity is guaranteed by construction.
