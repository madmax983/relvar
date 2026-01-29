## 2024-05-22 - Database God Object and Inconsistent Validation
**Learning:** `relvar-core/src/database.rs` is becoming a "God Object," handling all constraint logic (keys, FKs, checks, types) alongside transaction management and query execution. Additionally, `update` operations appear to bypass Type and Check constraints, unlike `insert`.
**Action:** Future refactors should consider extracting constraint validation into a dedicated `ConstraintValidator` or `IntegrityManager` component.
