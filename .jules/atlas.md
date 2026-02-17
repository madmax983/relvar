## 2025-05-24 - Extract ConstraintManager
**Tangle:** `Database` struct in `relvar-core/src/database.rs` was becoming a "God Struct", mixing database orchestration with constraint enforcement logic. The file was nearly 2000 lines long.
**Blueprint:** Extracted `ConstraintManager` and its error types into a dedicated `relvar-core/src/constraints/manager.rs` module. This improves cohesion by grouping constraint logic with constraint definitions and reduces coupling in the main database module.

## 2025-05-24 - Unify Database Error Handling
**Tangle:** `DatabaseError` duplicated `ConstraintManagerError` variants (violating DRY) and required manual synchronization.
**Blueprint:** Nested `ConstraintManagerError` within `DatabaseError` via a `Constraint(#[from] ConstraintManagerError)` variant. This enforces hierarchy and removes code duplication.

## 2025-05-24 - Decouple Query from Database
**Tangle:** Potential circular dependency between `Query` AST (in `relvar-core/src/query.rs`) and `Database` (in `relvar-core/src/database.rs`). `Query` depended on `Database` to execute, preventing `Database` from depending on `Query` (e.g. for stored view definitions).
**Blueprint:** Introduced `RelationSource` trait in `relvar-core/src/traits.rs`. `Query::execute` now depends on `RelationSource`, and `Database` implements it. This inverts the dependency, allowing `Database` to depend on `Query` in the future.

## 2025-05-24 - Promote Experimental Features
**Tangle:** Core relational features (`Delta`) and useful utilities (`Importer`, `Exporter`) were buried in `relvar/src/experimental`.
**Blueprint:**
- Moved `Delta` to `relvar-core/src/algebra/delta.rs` (high cohesion with other operators).
- Moved `Importer` and `Exporter` to `relvar/src/data/` (clear domain boundary for I/O).
