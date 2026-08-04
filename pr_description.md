Title: 🗺️ Atlas: [architectural change]

🕸️ Tangle: The `relvar-core/src/algebra/` module contained individual operator implementations in `.rs` files that mixed implementation logic and inline tests. Several files (like `summarize.rs`, `semijoin.rs`, `group.rs`) were growing into large God Files.
📐 Blueprint: Sub-modularized the algebra operators by creating dedicated subdirectories (e.g., `relvar-core/src/algebra/summarize/`) containing a `mod.rs` for the operator's implementation and a separate `tests.rs` for the tests, aligning with the project's architectural guidelines for sub-module structure.
🧱 Stability: Reduced file size bloat and isolated testing logic from core implementation, resulting in cleaner and more manageable code.
🔬 Verification: Tests pass successfully, module relationships remain intact, and formatting is enforced.
