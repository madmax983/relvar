# ⚒️ Forge: Extract `calculate_force_components` helper

🚮 Smell: Duplicate calculation for force components in `relvar/src/experimental/physics.rs`.
✨ Solution: Extracted the distance and force logic into a reusable helper function.
🧼 Benefit: Reduced code duplication and improved clarity by centralizing math computations.
🛡️ Verification: Tests passed. No logic changed.
