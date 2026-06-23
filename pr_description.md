🗺️ Atlas: Encapsulate experimental and tools modules

🕸️ Tangle: Broad visibility (`pub mod`) across many experimental and tools modules leaked internal implementations and created a sprawling public API surface.
📐 Blueprint: Converted `pub mod` to `pub(crate) mod` in `relvar/src/experimental/mod.rs`, `relvar-core/src/experimental/mod.rs`, and `relvar/src/tools/mod.rs` to hide internal hierarchies. Re-exported the necessary public types at the top level of these modules using the Facade pattern (`pub use module::*`). Updated tests to use the clean API.
🧱 Stability: Cleaned up public interfaces, enforced strong encapsulation, and improved structural cohesion.
🔬 Verification: All tests, doctests, and documentation build successfully without visibility errors.
