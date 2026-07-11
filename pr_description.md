🕸️ Tangle: Several core API files (e.g., `summarize.rs`, `semijoin.rs`, `scalar.rs`) had grown into massive God files containing over 800-1200 lines each, heavily bloated by inline test suites.
📐 Blueprint: Extracted the test suites into separate `tests.rs` files within dedicated modules (e.g., `summarize/mod.rs` and `summarize/tests.rs`), adhering to the Facade pattern for internal testing.
🧱 Stability: Reduced file coupling, improved modularity and maintainability by isolating tests from pure domain logic.
🔭 Verification: `cargo test` and `cargo check` build successfully. The files are now much shorter and easier to navigate.
