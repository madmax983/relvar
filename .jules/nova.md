## Relational Graph Analytics
**Concept:** Implemented `Graph` struct and algorithms (BFS, PageRank) using purely relational algebra (Join, Union, Difference, Summarize).
**Fate:** Merged
**Lesson:** Relational algebra is surprisingly capable of expressing iterative graph algorithms like BFS and PageRank, though it requires careful thinking about "set difference" to handle visited nodes correctly.

## Relational Linear Algebra
**Concept:** Modeled sparse matrices as relations with `row`, `col`, and `val` attributes. Implemented matrix addition and multiplication using standard relational algebra operators like Join, Extend, Summarize, and Union.
**Fate:** Merged
**Lesson:** Mathematical operations from linear algebra can map quite elegantly to relational operations, though handling multiplicities in operations like addition (where set semantics naturally deduplicate) requires careful tagging to distinguish sources before aggregation.

## Relational Collaborative Filtering Recommender
**Concept:** Implemented a User-User Collaborative Filtering recommender system using purely relational algebra operators like Join, Extend, Summarize, and Difference. It computes similarities using dot products and weights unseen items to generate predictions.
**Fate:** Pending
**Lesson:** Relational models can efficiently represent machine learning operations by leveraging Set operations and standard aggregations. The lack of standard array structures means that vector arithmetic like dot products must be expressed as joins and aggregations on relations.
