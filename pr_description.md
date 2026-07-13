🕸️ Tangle: `PageId` was just a primitive type alias (`type PageId = u64`), allowing unintended type casting and weakening the compiler's ability to enforce logical correctness between page IDs, log sequence numbers, and byte counts.

📐 Blueprint: Introduced the NewType pattern (`pub struct PageId(pub u64);`) to encapsulate the page identifier, preventing implicit conversions and enforcing strict domain boundaries at compile-time.

🧱 Stability: Enhanced type safety throughout the storage layer; prevents confusing a page identifier with raw size offsets or primitive values.

🔬 Verification: Builds successfully, all storage tests pass, strict separation enforced.
