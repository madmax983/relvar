🚮 Smell: The `compute_pairwise_forces` method in the relational physics engine was a "God Function" (80+ lines). It mixed joining relations, applying restrictions, and executing extensive force calculations in closure bodies.
✨ Solution: Extracted the logic into clear helper methods: `generate_particle_pairs`, `apply_force_extensions`, and `extract_force_components`.
🧼 Benefit: Flattens the logic, improves readability drastically, and isolates distinct relational operators and calculations.
🛡️ Verification: Tests passed. No logic changed.
