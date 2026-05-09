🕸️ Tangle: The `relvar-core/src/database/tests/mod.rs` module leaked its internal `common` testing setup module publicly via `pub mod common;`.
📐 Blueprint: Changed the visibility of the `common` testing module to `pub(crate) mod common;`, properly encapsulating internal test utilities while adhering to strict structural boundaries.
🧱 Stability: Prevents internal testing utilities from leaking, establishing a cleaner boundary within the core database implementation.
🔬 Verification: Builds successfully (`cargo check`), tests pass (`cargo test`), and strictly adheres to formatting and clippy lints (`cargo clippy --all-targets --all-features -- -D warnings`).
