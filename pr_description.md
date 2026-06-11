🗺️ Atlas: Sub-modularizing the HeapFile Blob
🕸️ Tangle: The `relvar-storage/src/storage/heap/mod.rs` module had grown into a massive 1500+ line God File (Blob anti-pattern) mixing physical tuple layout logic, constants, and complex MVCC insertion/visibility routines.
📐 Blueprint: Extracted pure data structures (`TupleId`, `SlotEntry`, `SlottedPage`, etc.), layout constants, and serialization helpers into a new dedicated internal submodule `layout.rs` and re-exported them in `mod.rs`. This enforces a clean Facade pattern, separating the layout from the operational logic.
🧱 Stability: Reduced coupling, faster compile times, improved readability.
🔬 Verification: Builds successfully, strict separation enforced. Tests pass successfully.
