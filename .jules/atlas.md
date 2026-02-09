## 2025-05-24 - Extract ConstraintManager
**Tangle:** `Database` struct in `relvar-core/src/database.rs` was becoming a "God Struct", mixing database orchestration with constraint enforcement logic. The file was nearly 2000 lines long.
**Blueprint:** Extracted `ConstraintManager` and its error types into a dedicated `relvar-core/src/constraints/manager.rs` module. This improves cohesion by grouping constraint logic with constraint definitions and reduces coupling in the main database module.

## 2025-05-24 - Unify Database Error Handling
**Tangle:** `DatabaseError` duplicated `ConstraintManagerError` variants (violating DRY) and required manual synchronization.
**Blueprint:** Nested `ConstraintManagerError` within `DatabaseError` via a `Constraint(#[from] ConstraintManagerError)` variant. This enforces hierarchy and removes code duplication.

## 2025-05-24 - Extract QueryExecutor and VirtualRelvarDefinition
**Tangle:** `Database` and `VirtualRelvarDefinition` had a circular dependency (tight coupling) because `VirtualRelvarDefinition` stored a function pointer taking `&mut Database<E>`. This prevented extracting `VirtualRelvarDefinition` to a separate module and forced it to be generic over `StorageEngine`.
**Blueprint:** Introduced `QueryExecutor` trait in `relvar-core/src/traits.rs`. Changed `VirtualRelvarDefinition` to use `fn(&mut dyn QueryExecutor)` and moved it to `relvar-core/src/virtual_relvars.rs`. Implemented `QueryExecutor` for `Database<E>`. This broke the cycle, removed the generic parameter from `VirtualRelvarDefinition`, and enabled mocking for virtual relvar tests.
