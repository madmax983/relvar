🗺️ Atlas: Fix public module leak in query tests

🕸️ Tangle
Broad visibility (`pub mod`) across the `relvar-core/src/query/tests` module leaked implementation details and complicated the dependency graph. `relvar-core/src/query/tests/mod.rs` had `pub mod` declarations that were leaking internal submodules.

📐 Blueprint
Converted the top-level internal module definitions in `relvar-core/src/query/tests/mod.rs` to `pub(crate) mod` to enforce clear boundaries and prevent leaky abstractions.

🧱 Stability
Reduces coupling and enforces strict separation by restricting visibility of the `tests` submodules to the crate level, aligning with high cohesion and low coupling principles.

🔬 Verification
The codebase was verified with `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all`. Builds successfully and tests pass, demonstrating the boundaries are sound.
