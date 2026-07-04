🌟 Nova: Relational Decision Tree Inference

💡 **The Spark:** I noticed we use relations for clustering and linear models, but we don't have a way to perform branching logic for machine learning inference like Decision Trees!

🚀 **The Feature:** Implemented `DecisionTree` in `src/experimental/decision_tree.rs`. It models `Nodes` and `Inputs` as relations, and evaluates a batch of inputs in parallel using relational algebra (Joins, Extends, and Restrictions) until all inputs reach a leaf node.

🔮 **The Potential:** This allows executing entire random forests or decision tree inference workloads as pure database queries, maximizing parallel batch processing natively!

⚠️ **Risk:** Low. The feature is purely additive and isolated within the `experimental` module.
