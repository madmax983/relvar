🚮 Smell: Deeply nested closures and long procedural blocks making experimental algorithms hard to parse.
✨ Solution: Extracted local closures (`evaluate_formula`, `compute_force`, etc.) to flatten `extend` / `restrict` pipelines across physics, graph, image, and genetic modules.
🧼 Benefit: Reduced cognitive load by decoupling computation logic from the relational algebra pipeline.
🛡️ Verification: Tests passed. No logic changed.
