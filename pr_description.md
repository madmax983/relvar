# ⚒️ Forge: Refactor bench_insert_with_check_constraint

🚮 Smell: The `bench_insert_with_check_constraint` in `benches/database.rs` was an overly long "God Function" combining complex database setup logic and inline benchmark execution, making the code hard to read and difficult to reason about.
✨ Solution: Extracted the initialization processes into distinct, well-named helper functions (`setup_simple_check_db` and `setup_complex_check_db`) and isolated the actual insert benchmark logic (`execute_insert_benchmark`).
🧼 Benefit: By separating the environment setup from the tight inner loop of the benchmark, the `bench_insert_with_check_constraint` function is dramatically flattened and simpler to read. The boundary between setup overhead and benchmark performance measurement is explicit.
🛡️ Verification: Cargo `fmt`, `clippy`, and `test` complete successfully with zero behavior changes or warnings.
