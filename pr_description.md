🗺️ Atlas: [architectural change] Sub-modularizing the HeapFile Blob

🕸️ Tangle: The `relvar-storage/src/storage/heap/mod.rs` module had grown into a massive Blob (>1500 lines), mixing pure internal page layout data structures with complex I/O, serialization, and MVCC transaction logic. This hurt readability, cohesion, and violated the Single Responsibility Principle.

📐 Blueprint: Extracted the page and slot data structures (`SlottedPage`, `VersionedSlottedPage`, `SlotEntry`, `VersionedSlotEntry`, `TupleId`) along with helper serialization logic and constants into a new `pub(crate) mod layout;` submodule. The orchestrating `mod.rs` acts purely as an operational facade for the `HeapFile` public API by internally re-using `layout::*`.

🧱 Stability: Improved module cohesion by strictly separating "what data looks like" (layout) from "how data is manipulated" (HeapFile operations). No changes to the public storage engine API.

🔬 Verification: Builds successfully, all `relvar-storage` tests pass perfectly, and the internal dependency boundaries strictly encapsulate `layout` within `heap`.
