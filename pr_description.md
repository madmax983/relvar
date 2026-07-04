🗺️ Atlas: [Module Privacy Cleanup]

🕸️ Tangle: The codebase had multiple public module declarations (`pub mod`) across internal namespaces (e.g. `relvar-core/src/database/tests/mod.rs`, `relvar-core/src/query/tests/mod.rs`, `relvar-storage/src/persistent_engine/tests/common.rs`, `relvar-core/src/query/tests/common.rs`), leaking test infrastructure and common test helper functions as public APIs, resulting in poor encapsulation and muddying dependency boundaries.
📐 Blueprint: Converted public module definitions in test and internal directories to private (`mod`) or crate-private (`pub(crate) mod`). Modified shared test helper functions from `pub fn` to `pub(crate) fn` to ensure test implementations remain fully internal, correctly separating the public contract from test artifacts.
🧱 Stability: Strict separation enforced; test utilities are no longer accidentally leaked to consumers. Faster compile times and cleaner IDE discovery.
🔬 Verification: Builds successfully, all tests pass, and strictly adheres to the rule of using `pub(crate)` by default for modules.
