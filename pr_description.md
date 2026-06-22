🪒 Razor: Flatten GraphNeuralNetwork namespace

💡 **The Spark:** The `GraphNeuralNetwork` unit struct was being used purely as a Java-style class namespace for a single static `forward` method, which is an anti-pattern in Rust.
🚀 **The Feature:** Removed the unit struct and converted `forward` into a simple top-level free function within the `gnn` module.
🔭 **The Potential:** Reduces boilerplate and cognitive load by removing an unnecessary abstraction layer.
⚠️ **Risk:** Low. Only the invocation syntax changes.
