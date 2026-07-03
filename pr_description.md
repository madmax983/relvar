🗺️ Atlas: [Test Module Encapsulation]

🕸️ Tangle: Internal test sub-modules in `relvar-core/src/query/tests/mod.rs` (`basic` and `common`) were exposed as `pub mod`, leaking test implementation details.
📐 Blueprint: Changed the module declarations to private (`mod`) to enforce strict boundaries and prevent leaking test details.
🧱 Stability: Reduced coupling by encapsulating test internals.
🔬 Verification: Builds successfully, strict separation enforced.
