🗺️ Atlas: Modularization of HeapFile Blob

🕸️ Tangle: The `relvar-storage/src/storage/heap/mod.rs` module had become a massive 1500+ line God Module containing basic struct definitions, insertion, reading, updating, MVCC mechanics, garbage collection, and page serialization. It exhibited "The Blob" anti-pattern and suffered from low cohesion.

📐 Blueprint: Extracted the implementations into a robust module structure: `insert.rs`, `read.rs`, `update.rs`, `gc.rs`, and `serialization.rs`. Converted internal fields like `page_file` and `relation_type` to `pub(crate)` where necessary so sub-modules can execute their specific domains while keeping implementation details perfectly hidden from external crates. The `mod.rs` file now acts merely as a clean facade.

🧱 Stability: Enforces strict separation of concerns within the storage heap subsystem, drastically improving readability, long-term maintainability, and decreasing compiler dependency debt.

🔬 Verification: The codebase compiles correctly without any circular dependency warnings. Cargo tests and clippy pass perfectly across all targets and features.
