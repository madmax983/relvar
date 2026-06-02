🗺️ Atlas: Extract HeapFile page format structs into internal submodule

🕸️ Tangle: The `HeapFile` module (`relvar-storage/src/storage/heap/mod.rs`) was a massive Blob of 1500+ lines, mixing page format representations (`SlottedPage`, `VersionedSlottedPage`, `SlotEntry`) alongside operation logic.
📐 Blueprint: Extract page layout structs (`SlotEntry`, `VersionedSlotEntry`, `SlottedPage`, `VersionedSlottedPage`) and their serialization logic into a dedicated internal submodule (`page_format.rs`), then re-export them into `mod.rs`. This isolates the data formats from operational implementation.
🧱 Stability: Reduced coupling, separates data from operations.
🔬 Verification: Builds successfully, strict separation enforced.
