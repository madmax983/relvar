🗺️ Atlas: [architectural change] refactor heap storage into submodules

💡 **The Spark:**
The `relvar-storage/src/storage/heap/mod.rs` file was a massive "Blob" module (over 1500 lines) containing error definitions, page layout structures, and the main `HeapFile` implementation. This made the module hard to navigate, maintain, and violated the principle of high cohesion.

🚀 **The Feature:**
Refactored the monolithic `heap/mod.rs` into smaller, cohesive modules:
- Extracted error types and serialization helpers into `error.rs`
- Extracted slotted page layouts and structures into `page.rs`
- Extracted the main `HeapFile` implementation into `heap_file.rs`
- Updated `mod.rs` to serve purely as a facade, re-exporting the necessary types to maintain the existing public API contract.

🔭 **The Potential:**
This architectural change improves maintainability and enforces strict separation of concerns within the storage layer without breaking any external dependencies. It ensures that changes to the page layout or error handling do not require navigating through thousands of lines of `HeapFile` logic.

⚠️ **Risk:**
Low. The restructuring is purely internal to `relvar-storage/src/storage/heap/`. All visibility and public API contracts remain identical, verified by the existing comprehensive test suite.
