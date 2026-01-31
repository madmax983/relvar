## 2024-05-22 - Database God Object and Inconsistent Validation
**Learning:** `relvar-core/src/database.rs` is becoming a "God Object," handling all constraint logic (keys, FKs, checks, types) alongside transaction management and query execution. Additionally, `update` operations appear to bypass Type and Check constraints, unlike `insert`.
**Action:** Future refactors should consider extracting constraint validation into a dedicated `ConstraintValidator` or `IntegrityManager` component.

## 2024-05-23 - Constraint Logic Extraction Success
**Learning:** Extracting constraint logic from `Database` to `ConstraintManager` significantly reduced the size and complexity of the main struct. Passing `&mut Engine` explicitly to the manager's methods resolved potential borrowing conflicts.
**Action:** Use the "Manager" pattern with explicit dependency injection (passing `&mut Dependency` to methods) when extracting logic that requires access to sibling fields.
