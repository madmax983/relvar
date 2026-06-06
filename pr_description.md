🗺️ Atlas: [fix public mod leaks]

🕸️ Tangle: The codebase had multiple public module leaks across `relvar`, `relvar-core`, and `relvar-storage`, exposing internal implementation details, particularly in the `experimental` modules and `tools`. Also, the `tools` internal components were hidden from the `pub use tools::*` re-exports in `relvar/src/lib.rs` due to incorrect `pub(crate)` declarations.
📐 Blueprint: Applied `pub(crate) mod` and `#[allow(dead_code)]` to the internal experimental and tools modules. Updated `relvar/src/lib.rs` to use `pub use crate::tools::*` so that the intended tools API is correctly exported. Adjusted tests to import types from the top-level instead of private internal modules.
🧱 Stability: Improved encapsulation and reduced coupling. Cleaned up compiler warnings.
🔭 Verification: `cargo clippy`, `cargo test`, and `cargo test --doc` run without warnings or errors.
