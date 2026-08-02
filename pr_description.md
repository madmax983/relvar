Title: 🗺️ Atlas: [Sub-modularizing the Algebra Tests Blob]

🕸️ Tangle: The `relvar-core/src/algebra/` files had grown massive, acting as "God Files" because they contained massive inline `#[cfg(test)] mod tests { ... }` blocks that accounted for hundreds of lines of code. This made navigating the actual relational algebra implementation logic extremely difficult.

📐 Blueprint: Extracted the tests into dedicated `tests.rs` files inside specialized sub-directories for each operator (e.g., `relvar-core/src/algebra/semijoin/tests.rs` and `relvar-core/src/algebra/semijoin/mod.rs`), effectively breaking the "Blob" anti-pattern and enforcing structural separation of concerns between implementation and testing code.

🧱 Stability: Reduced file bloat significantly, improving readability, navigation, and long-term maintainability without changing any functionality.

🔬 Verification: Builds successfully, all tests pass, and strict separation is enforced.
