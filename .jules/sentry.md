## 2026-05-01 - Tuple Grouping and Summarization Attribute Not Found Error Maps
**Learning:** Functions like `group`, `summarize`, and `ungroup` internally iterated and used `.unwrap()` on `tuple.get(attr)` under the assumption that the validated relation type strictly guaranteed attribute presence. While this is mathematically true in the pure relational model, any internal inconsistency, malformed tuples bypassing validation, or memory corruption could trigger an unhandled panic in production.
**Action:** Replaced `.unwrap()` calls in `group.rs` and `summarize.rs` with `.ok_or_else(|| ...)` explicitly mapping to `GroupError::AttributeNotFound` and `SummarizeError::AttributeNotFound`, respectively. Always prefer robust Result propagation over panics, even for seemingly "guaranteed" schema invariants.
## 2026-05-05 - DML Constraints Logic Coverage

**Learning:** Missing coverage branches for specific operations involving database bulk constraints and cascading failure. `compute_relation_after_update` early exit logic via `?` unrolls some lines in `Database::update` if the logic before reaches an error state.

**Action:** When adding tests for database constraints involving `update` and `delete`, ensure varying degrees of constraint violations such as duplicated primary keys after updates and violations on foreign keys mapped against parent relations to fully hit bulk checks.
