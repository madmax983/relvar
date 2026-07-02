🛡️ Sentry: [test coverage improvement]

🎯 Target: `relvar-core/src/algebra/*` (group, extend, difference, intersect, join, project, rename, semijoin, union, summarize)
💣 Risk: Missing test coverage for complex error conditions and edge-cases related to types, unhandled validation states, fast-path optimizations for empty relations. Unhandled cases could lead to logic bugs or unhandled errors.
🧪 Strategy: Consolidated testing to cover type-mismatches across `extend`, `difference_into`, `union_into`, empty-relation optimizations (`intersect`, `difference`, `union`, `semijoin`), and various unhandled edge cases directly in algebra primitives.
🔬 Verification: Run `cargo test --test sentry_algebra_uncovered` and `cargo tarpaulin` to confirm structural bounds are evaluated properly.
