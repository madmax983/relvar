🗺️ Atlas: [fix query test module boundary]

🕸️ Tangle: The `relvar-core/src/query/tests/mod.rs` file was exposing its sub-modules publicly via `pub mod`, which leaked test implementation details.
📐 Blueprint: Converted the sub-module declarations from `pub mod` to private `mod` in `relvar-core/src/query/tests/mod.rs` to enforce proper module boundaries.
🧱 Stability: Prevented test internals from leaking, ensuring strict module encapsulation.
🔬 Verification: `cargo test` and `cargo check` pass successfully.
