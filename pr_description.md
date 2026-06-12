🗺️ Atlas: Sub-modularize HeapFile Blob

🕸️ Tangle: `relvar-storage/src/storage/heap/mod.rs` was a massive Blob anti-pattern file containing both pure data structure definitions (like `TupleId`, `SlottedPage`, `HeapError`, etc.) and complex multi-version concurrency, indexing, and I/O logic.

📐 Blueprint: Extracted pure data structures and constants into a dedicated internal submodule `types.rs`. Re-exported them via `mod.rs` to flatten boundaries, isolating data definitions from operational implementations and applying the Facade pattern.

🧱 Stability: Reduced file bloat and improved module cohesion. Internal details stay private while the public API remains unchanged.

🔬 Verification: Builds successfully, all tests pass, zero linter warnings.
