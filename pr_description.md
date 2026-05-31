# 🗺️ Atlas: [HeapFile Extracted Page Format]

**🕸️ Tangle:**
The `HeapFile` module in `relvar-storage/src/storage/heap/mod.rs` had grown into a God Module (Blob anti-pattern) exceeding 1,500 lines. It mixed the pure data structures (like page formats and their serialization bounds) alongside the operational logic (inserting tuples, maintaining files, and scanning), violating the Single Responsibility Principle and muddling dependencies.

**📐 Blueprint:**
Extracted the pure data structures (`TupleId`, `SlotEntry`, `VersionedSlotEntry`, `SlottedPage`, `VersionedSlottedPage`), serialization constants, and formatting helper functions into a dedicated internal submodule `page_format.rs`. These are re-exported using `pub(crate) mod page_format` and `pub(crate) use page_format::*` within the `mod.rs` file. This flattens the boundaries while isolating data representation from the operational storage implementation.

**🧱 Stability:**
This decoupling drastically improves modularity. The operational `HeapFile` implementation is now less cluttered, and the page format can be extended independently of physical disk operations. We avoid compilation issues by leaving all components marked `pub(crate)` allowing seamless internal accessibility.

**🔬 Verification:**
- Executed `cargo check` to verify the module boundaries and crate visibility.
- Ran `cargo test` to ensure that standard serialization testing and data consistency are unaffected by the separation.
- Verified successful extraction with `cargo clippy --all-targets --all-features -- -D warnings`.
- Documented in `.jules/atlas.md`.
