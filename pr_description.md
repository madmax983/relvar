🕸️ Tangle: Broad visibility (`pub mod`) across test and internal modules leaked implementation details and complicated the dependency graph. Specifically, `relvar-core/src/query/tests/mod.rs` was exposing internal testing modules.
📐 Blueprint: Converted the internal module definitions in `relvar-core/src/query/tests/mod.rs` to `mod` and `pub(crate) mod` to restrict visibility to the crate, enforcing clean, intention-revealing public APIs while maintaining low coupling between internal components.
🧱 Stability: Reduced coupling by encapsulating internal logic. Faster compile times and clear module boundaries.
🔬 Verification: Builds successfully, all tests pass, and strict separation is enforced.
