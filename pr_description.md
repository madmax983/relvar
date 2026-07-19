🚮 Smell: The `compute_pairwise_forces` method in the relational physics engine was a "God Function" (80+ lines). It mixed joining relations, applying restrictions, and executing extensive force calculations in closure bodies. The timeseries module also contained an issue failing clippy tests.
✨ Solution: Extracted the logic in `compute_pairwise_forces` into clear helper methods: `generate_particle_pairs`, `apply_force_extensions`, and `extract_force_components`. Also fixed a `clippy::for_kv_map` warning in `timeseries.rs`.
🧼 Benefit: Flattens the logic, improves readability drastically, and isolates distinct relational operators and calculations, while ensuring all code passes CI cleanly.
🛡️ Verification: Tests passed. No logic changed.
