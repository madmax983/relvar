🌟 Nova: Relational Decision Tree Inference

💡 **The Spark:** "We can classify tabular data natively inside the database!" I noticed we evaluate relational models on vectors but lack a native tree inference model for structured features.

🚀 **The Feature:** Implemented `DecisionTree` inference evaluating pure declarative paths over relations. Using Join, Restrict, and Extend, we elegantly traverse nodes until predicting target classes—completely natively!

🔮 **The Potential:** Can be used to build a fully Relational Machine Learning suite! Allowing users to run pre-trained Random Forests entirely inside the query planner!

⚠️ **Risk:** Low. Isolated in `src/experimental/decision_tree.rs`.
