# ⚒️ Forge: Extract God Function in Physics Simulation and Fix Clippy Lints

🚮 Smell:
1. The `compute_pairwise_forces` method in `relvar/src/experimental/physics.rs` was an 81-line "God Function" that handled joining, filtering, and performing two massive extensions for computing X and Y force components in one continuous block.
2. In `relvar/src/experimental/timeseries.rs`, `for (attr_name, _) in original_heading.attributes().iter()` was used instead of `.keys()`, triggering `clippy::for-kv-map`.

✨ Solution:
1. Applied the "Three-Phase Operator" pattern by extracting `generate_particle_pairs`, `filter_self_interactions`, and `calculate_force_components` into separate private helper functions.
2. Replaced `.iter()` with `.keys()` in `timeseries.rs` to fix the clippy lint.

🧼 Benefit:
1. Significantly flattens the main control flow, making the relational physics steps highly readable without altering any underlying behavior.
2. Adheres to idiomatic Rust patterns.

🛡️ Verification: Tests and Clippy checks passed. No logic changed.
