## 2025-05-24 - Extract ConstraintManager
**Tangle:** `Database` struct in `relvar-core/src/database.rs` was becoming a "God Struct", mixing database orchestration with constraint enforcement logic. The file was nearly 2000 lines long.
**Blueprint:** Extracted `ConstraintManager` and its error types into a dedicated `relvar-core/src/constraints/manager.rs` module. This improves cohesion by grouping constraint logic with constraint definitions and reduces coupling in the main database module.
