🗺️ Atlas: [Test Module Encapsulation]

🕸️ Tangle: Internal test sub-modules (`basic` and `common` in `relvar-core/src/query/tests/mod.rs`) were exposed via `pub mod`, leaking test implementation details.
📐 Blueprint: Changed the visibility of these modules to `pub(crate) mod` to enforce strict module boundaries and prevent test leakage.
🧱 Stability: Strict separation enforced, keeping internal test structures completely hidden.
🔬 Verification: `cargo test` and `cargo doc` passed without issue, ensuring nothing in the rest of the workspace relied on this leaked API.
