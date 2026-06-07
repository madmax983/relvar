🌟 Nova: Relational Linear Regression

💡 **The Spark**: Machine learning algorithms fundamentally rely on linear algebra and aggregation. Can we implement Gradient Descent directly inside the relational engine?
🚀 **The Feature**: Implemented `LinearRegression` using purely relational algebra. Features, Targets, and Weights are modeled as relations, and the training loop executes forward and backward passes using Join, Extend, and Summarize (Aggregation).
🔭 **The Potential**: This proves that iterative numerical optimization models can be trained and evaluated declaratively inside the database execution engine without exporting data.
⚠️ **Risk**: Very low. It is isolated as a self-contained feature within `src/experimental/linear_regression.rs`.
