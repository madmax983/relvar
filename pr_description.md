⚒️ Forge: Extract God Functions in Raytracer

🚮 Smell: The `compute_intersections` and `calculate_visible_pixels` functions in `relvar/src/experimental/raytracer.rs` were long "God Functions" (over 65 lines each). They mixed multiple steps of relational operations (cross join, math calculations for distances, filtering hits, z-buffer logic, and background colors) into massive blocks of code, making them difficult to follow.

✨ Solution: Refactored these functions by applying the "Three-Phase Operator" pattern. Extracted logical phases into focused, private helper functions: `prepare_spheres`, `calculate_intersection_distances`, `filter_hits`, `find_closest_hits`, `map_colors`, and `apply_background`.

🧼 Benefit: Flattens the execution flow of the main methods. Each helper function now clearly states its relational intent and handles a specific phase, drastically reducing cognitive load and improving readability without altering runtime behavior.

🛡️ Verification: Tests passed. No logic changed.
