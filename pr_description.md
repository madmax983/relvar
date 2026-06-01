🗺️ Atlas: [Extract HeapFile Page Formats]

🕸️ Tangle: The `relvar-storage/src/storage/heap/mod.rs` file had grown into a massive Blob, mixing functional logic with purely structural data format definitions (`SlottedPage`, etc.).
📐 Blueprint: Extracted page format structures and constants into a new internal submodule `page_format.rs` and re-exported them, separating data layout from heap operational logic.
🧱 Stability: No change to logic, just better structural separation.
🔬 Verification: All tests and linters pass.
