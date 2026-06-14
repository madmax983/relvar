🪒 Razor: Eliminate Factory Factory in MockRelation Builder
💡 **The Spark:**
The `MockRelation` builder pattern in `relvar/src/experimental/mock.rs` is a classic "Factory Factory". It forces developers to instantiate a builder, configure it via chained methods (`count()`, `seed()`), and then call `generate()`.

🚀 **The Feature:**
Refactored `MockRelation` from a stateful builder struct into a stateless unit struct with a direct `generate()` function. This eliminates the unnecessary intermediate struct and provides a cleaner, functional API.

🔮 **The Potential:**
Simpler mock data generation code throughout the test suite.

⚠️ **Risk:**
None, this is a local refactoring in an experimental module. Tests pass perfectly.
