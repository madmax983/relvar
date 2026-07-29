Title: Fix: Resolve unused import warnings in relvar-storage and vector_db

This commit fixes CI failures by removing unused imports (`RelationMetadata`, `PAGE_SIZE`, `PageId`) from `relvar-storage/src/storage/mod.rs` and unused imports (`RelationType`, `TupleType`) from `relvar/src/experimental/vector_db.rs`. It also adds `#[allow(dead_code)]` to the newly added `VectorDB` struct to suppress dead code warnings. These changes ensure `cargo clippy --all-targets --all-features -- -D warnings` completes successfully.
