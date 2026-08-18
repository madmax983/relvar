# 🌟 Nova: Relational Graph Neural Network

💡 **The Spark:** "Can we run a Graph Neural Network (Message Passing) layer natively using only Relational Algebra operators?" By utilizing relational operations like natural join and aggregate summarization, the core database engine can act directly as an inference engine for Graph ML.

🚀 **The Feature:** "Implemented `mpnn_layer` which resolves one full layer of a Message Passing Neural Network (MPNN) by iteratively using Natural Joins to pass messages along edge relations, aggregating neighbor features via `summarize` (with `Aggregation::sum_float`), and computing the linear transformation using `join` and `extend`."

🔮 **The Potential:** "This completely unlocks graph learning! Database tables representing features and nodes can now transparently perform Graph ML inference directly in the query layer without needing an external processing system like PyTorch."

⚠️ **Risk:** "Low. The feature is completely isolated inside the `src/experimental/graph_neural_network.rs` module behind the `nova` feature flag, and strictly uses existing Relational primitives (Join, Summarize, Extend)."
