🕸️ Tangle: The files `relvar-core/src/algebra/summarize.rs`, `relvar-core/src/algebra/semijoin.rs`, and `relvar-core/src/values/scalar.rs` had grown into 'God Files' containing over a thousand lines each. The inline tests were mixed directly into the implementation source, violating the modular separation of concerns and hindering navigation.

📐 Blueprint: Refactored these files into sub-modules (e.g., `relvar-core/src/algebra/summarize/`, `relvar-core/src/algebra/semijoin/`, `relvar-core/src/values/scalar/`). The implementations remain in `mod.rs` while the tests have been extracted out to dedicated `tests.rs` files using the `#[cfg(test)] mod tests;` idiom.

🧱 Stability: High cohesion, cleaner file structures, and strict separation between domain logic and tests. No change to external APIs or module behavior.

🔬 Verification: Builds successfully, all tests pass, and strictly adheres to the architectural guidelines.
