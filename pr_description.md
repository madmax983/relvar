# 🌟 Nova: Relational Recommender System

💡 **The Spark:** "I realized that collaborative filtering—computing cosine similarities and predicting ratings—can be entirely expressed using relational algebra. Why write custom ML loops when we can just use `Join`, `Extend`, and `Summarize`?"
🚀 **The Feature:** "Implemented a `recommender` module in `src/experimental/` that computes user-user similarities and predicts unseen item ratings using pure relational algebra."
🔭 **The Potential:** "This turns the database into a rudimentary recommendation engine. Imagine embedding suggestions directly into SQL-like queries without needing a separate ML pipeline!"
⚠️ **Risk:** "Low. Isolated in `src/experimental/recommender.rs` and behind the `nova` feature flag."
