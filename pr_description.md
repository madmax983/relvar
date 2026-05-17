🌟 Nova: Relational K-Nearest Neighbors (KNN)

💡 The Spark: We have complex operations like computing distances and determining majority votes in data science. Can we implement a lazy learning algorithm like K-Nearest Neighbors purely with relational algebra without procedural loops?

🚀 The Feature: Implemented the `KnnClassifier` in `src/experimental/knn.rs`. It calculates distances using `Extend`, uses Theta-Joins coupled with counting logic (`Summarize`) to rank neighbors declaratively (Top-N pattern), and employs another Top-1 declarative query to determine the majority class vote among the K nearest neighbors.

🔭 The Potential: This expands the potential of Relvar by showing that it can serve not just as a database engine, but as an ML inference engine capable of directly executing non-parametric algorithms like KNN natively on its tables.

⚠️ Risk: Low. Entirely isolated behind the `experimental/` directory and doesn't mutate or affect any core logic.
