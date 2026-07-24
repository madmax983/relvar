💡 **The Spark:** "We have search capabilities, and we can perform linear algebra natively using Joins and Aggregations... Can we implement a Vector Database using pure relational algebra?"

🚀 **The Feature:** "Implemented `VectorDatabase` in `src/experimental/vector_search.rs`. This implements a dot product similarity search by representing document embeddings and search queries as relations. By joining on the dimension index, extending the relation with the pairwise product, and summarizing via `Aggregation::sum_float`, we can perform dot product similarity searches completely declaratively using the existing Relvar algebraic operators!"

🔭 **The Potential:** "This proves that Vector Databases and retrieval-augmented generation (RAG) models can operate cleanly on top of a purely relational foundation without requiring custom, opaque distance-calculating functions outside the database engine. We've bridged unstructured AI embeddings with the structural rigor of TTM!"

⚠️ **Risk:** "Low. Isolated in `src/experimental/vector_search.rs`."
