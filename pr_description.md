🗺️ Atlas: [architectural change] Fixing public module leaks in query tests

🕸️ Tangle: The `relvar-core/src/query/tests/mod.rs` module was publicly exposing its internal testing submodules (`basic`, `common`) using `pub mod`, which leaks implementation details.
📐 Blueprint: Converted `pub mod basic;` and `pub mod common;` to `mod basic;` and `mod common;` to prevent leaking internal test structures.
🧱 Stability: Reduced coupling, stricter separation enforced.
🔬 Verification: Builds successfully, strict separation enforced.
