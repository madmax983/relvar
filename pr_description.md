💡 **The Spark:** I noticed we didn't have any features dealing with spatial data or machine learning capabilities like embeddings search. I realized that a vector database for similarity search can be elegantly expressed using purely relational algebra operators like `Join`, `Extend`, and `Summarize` without needing custom index structures.

🚀 **The Feature:** Implemented `VectorDatabase` with methods `dot_product_search` and `l2_distance_search` to find the similarity between relations representing high-dimensional vectors.

🔮 **The Potential:** Could be used for relational embeddings search, retrieval-augmented generation (RAG) directly in the database, and content recommendation engines.

⚠️ **Risk:** Low. The feature is completely isolated in `src/experimental/vector_search.rs`.
