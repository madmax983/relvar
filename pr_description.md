🗺️ Atlas: [architectural change] Fixing public module leak in query tests

🕸️ Tangle: The query tests module incorrectly exposed `basic` and `common` submodules as `pub mod`, leaking test details into the public API.
📐 Blueprint: Changed `pub mod basic` to `mod basic` and `pub mod common` to `pub(crate) mod common` to restrict visibility strictly to the test harness.
🧱 Stability: Reduced coupling, cleaner public API boundary.
🔬 Verification: Builds successfully, strict separation enforced.
