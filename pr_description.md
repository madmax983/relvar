🚮 Smell: The `compute_pairwise_forces` method in `relvar/src/experimental/physics.rs` was 81 lines long and duplicated the exact same mathematical logic for calculating distances and force components twice (once for the X component, once for the Y component) within chained `.extend()` calls.

✨ Solution: Extracted the duplicated mathematical operations into a private static helper function `compute_force_components(t: &relvar_core::values::Tuple, g: f64) -> (ScalarValue, ScalarValue)`. The chained `.extend()` calls now simply delegate to this helper and use `.0` and `.1` respectively.

🧼 Benefit: Dramatically reduces the size of `compute_pairwise_forces`, removes logic duplication according to DRY principles, fixes boolean blindness by returning both components together in a tuple, and makes the core physics calculation easier to read and test in isolation if needed. The execution flow is flattened.

🛡️ Verification: Tests passed (`cargo test`). No logic changed; the physics engine yields identical state progression as before.
