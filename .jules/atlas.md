## 2026-02-21 - Removed QueryExecutor Abstraction
**Tangle:** `VirtualRelvarDefinition` depended on `QueryExecutor` trait to avoid circular dependency with `Database` struct. This created an unnecessary abstraction layer (`QueryExecutor`) with only one real implementation (`Database`).
**Blueprint:** Removed `QueryExecutor` trait. Merged `virtual_relvar` module into `database/mod.rs` to resolve the resulting circular dependency between `Database` (which holds views) and `VirtualRelvarDefinition` (which queries `Database`). Updated `Query::execute` to depend directly on `Database<E>`.
