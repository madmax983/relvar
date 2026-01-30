## 2026-01-30 - Refactoring God Struct Database
**Tangle:** `Database<E>` in `relvar-core` was a God Struct mixing storage orchestration, transaction management, and system catalog (constraints, views). It had a circular dependency where `VirtualRelvarDefinition` depended on `Database` which depended on `VirtualRelvarDefinition` via the `evaluator` function pointer.
**Blueprint:**
1. Extracted `SystemCatalog` to `relvar-core/src/database/catalog.rs` to manage metadata and validation logic.
2. Created `QueryContext` trait in `relvar-core/src/database/traits.rs` to abstract query capability.
3. Updated `VirtualRelvarDefinition` to depend on `&dyn QueryContext` instead of `&mut Database<E>`, breaking the dependency cycle and removing the generic type parameter from the catalog.
4. Refactored `Database` to delegate constraint management to `SystemCatalog` and implement `QueryContext`.
