🚮 Smell: The `compute_pairwise_forces` method in the physics engine had deeply duplicated mathematical logic within its `.extend` closures for calculating X and Y force components, making it hard to read and a classic 'God Closure'. Passing raw true/false for axis differentiation also caused boolean blindness.
✨ Solution: Extracted the distance calculation and force formulation into a single, private `compute_force_component` helper function. Introduced an explicit `Axis` Enum to differentiate between X and Y axes instead of opaque booleans.
🧼 Benefit: Reduces cognitive load, strictly enforces DRY, and eliminates boolean blindness, making the formula logic understandable at a glance.
🛡️ Verification: Tests passed. No logic changed.
