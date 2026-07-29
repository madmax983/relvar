Title: 🌟 Nova: Relational Vector Database Simulator

💡 **The Spark:** "I noticed we use relational operations for set logic, but can they handle spatial math like dot products and magnitudes for embeddings? Can a database natively function as a Vector DB using pure relation joins?"

🚀 **The Feature:** "Implemented `VectorDB` struct using entirely relational algebra to compute Cosine Similarities between a query relation and embeddings stored in the DB (via Join, Extend, Summarize)."

🔭 **The Potential:** "Could be used natively for similarity search, AI/LLM retrieval tasks, recommendation engines, or spatial querying purely within the relational engine."

⚠️ **Risk:** "Low. Fully isolated in `src/experimental/vector_db.rs`."
