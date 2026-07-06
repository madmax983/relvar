🗺️ Atlas: [architectural change] Fixing Public Module Leaks in Query Tests

🕸️ Tangle: Broad visibility (`pub mod`) in `relvar-core/src/query/tests/mod.rs` leaked test implementation details, violating module encapsulation.
📐 Blueprint: Converted `pub mod` to `mod` for internal test sub-modules (`basic`, `common`), properly hiding internal test logic.
🧱 Stability: Reduced coupling, strict separation enforced.
🔬 Verification: Builds successfully, strict separation enforced.
