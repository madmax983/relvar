🚮 Smell: The `compute_pairwise_forces` and `update_kinematics_and_project` methods in `relvar/src/experimental/physics.rs` contained very large inline closures for their `extend` relational algebra operations. This created a "God Function" smell where complex mathematical logic was deeply nested and duplicated, making the relational pipeline hard to read.

✨ Solution: Applied the "Three-Phase Operator" and extraction patterns. Extracted the mathematical calculation logic inside the `extend` closures into isolated, private, cleanly typed helper functions on `PhysicsEngine` (`compute_force_x`, `compute_force_y`, `compute_new_vx`, `compute_new_vy`, `compute_new_x`, `compute_new_y`).

🧼 Benefit: Dramatically improves readability by flattening the nested closures. The physical calculation logic is now cleanly separated from the relational data flow pipeline. Reduces cognitive load.

🛡️ Verification: Tests passed. No logic changed.
