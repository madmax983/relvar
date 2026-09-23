//! Relational Vector Search (Vector Database)
//!
//! This module implements a Vector Database using purely relational algebra.
//! It demonstrates that dense vector embeddings and similarity search (e.g., Dot Product)
//! can be expressed elegantly using relational Joins, Extensions, and Aggregations.
//!
//! # Concept
//!
//! - **Embeddings**: Relation `(doc_id: Int, dim: Int, value: Float)`.
//! - **Query**: Relation `(dim: Int, query_value: Float)`.
//!
//! The algorithm for Dot Product similarity:
//! 1. Join `Embeddings` and `Query` on `dim`.
//! 2. Extend the joined relation with `product = value * query_value`.
//! 3. Summarize by `doc_id` with a float sum over `product` to get the `score`.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Vector Database.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::vector_search::VectorDatabase;
///
/// let emb_type = TupleType::new()
///     .with_attribute("doc_id", ScalarType::Int)
///     .with_attribute("dim", ScalarType::Int)
///     .with_attribute("value", ScalarType::Float);
/// let embeddings = Relation::new(RelationType::new(emb_type));
///
/// let db = VectorDatabase::new(embeddings);
/// ```
pub struct VectorDatabase {
    /// The embeddings relation.
    /// Schema: (doc_id: Int, dim: Int, value: Float)
    pub embeddings: Relation,
}

impl VectorDatabase {
    /// Creates a new Vector Database.
    ///
    /// The `embeddings` relation should contain the vector components for all documents.
    pub fn new(embeddings: Relation) -> Self {
        Self { embeddings }
    }

    /// Performs a dot product search against the embeddings.
    ///
    /// The `query` relation must have the schema `(dim: Int, query_value: Float)`.
    /// Returns a relation with schema `(doc_id: Int, score: Float)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::vector_search::VectorDatabase;
    ///
    /// let emb_type = TupleType::new()
    ///     .with_attribute("doc_id", ScalarType::Int)
    ///     .with_attribute("dim", ScalarType::Int)
    ///     .with_attribute("value", ScalarType::Float);
    /// let mut embeddings = Relation::new(RelationType::new(emb_type));
    /// embeddings.insert(tuple! { doc_id: 1i64, dim: 0i64, value: 1.0f64 }).unwrap();
    /// embeddings.insert(tuple! { doc_id: 1i64, dim: 1i64, value: 0.0f64 }).unwrap();
    /// embeddings.insert(tuple! { doc_id: 2i64, dim: 0i64, value: 0.0f64 }).unwrap();
    /// embeddings.insert(tuple! { doc_id: 2i64, dim: 1i64, value: 1.0f64 }).unwrap();
    ///
    /// let query_type = TupleType::new()
    ///     .with_attribute("dim", ScalarType::Int)
    ///     .with_attribute("query_value", ScalarType::Float);
    /// let mut query = Relation::new(RelationType::new(query_type));
    /// query.insert(tuple! { dim: 0i64, query_value: 1.0f64 }).unwrap();
    /// query.insert(tuple! { dim: 1i64, query_value: 0.0f64 }).unwrap();
    ///
    /// let db = VectorDatabase::new(embeddings);
    /// let results = db.search_dot_product(&query).unwrap();
    ///
    /// // Doc 1 should have score 1.0, Doc 2 should have score 0.0
    /// assert_eq!(results.cardinality(), 2);
    /// ```
    pub fn search_dot_product(&self, query: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Join embeddings and query on `dim`
        let joined = self.embeddings.join(query)?;

        // 2. Compute component-wise product
        let with_product = joined
            .extend("product", ScalarType::Float, |t| {
                let val = t.get_typed::<f64>("value").unwrap_or(0.0);
                let q_val = t.get_typed::<f64>("query_value").unwrap_or(0.0);
                ScalarValue::Float(val * q_val)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by doc_id to get the final score
        let result = with_product
            .summarize(&["doc_id"], &[Aggregation::sum_float("score", "product")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    fn setup_embeddings() -> Relation {
        let emb_type = TupleType::new()
            .with_attribute("doc_id", ScalarType::Int)
            .with_attribute("dim", ScalarType::Int)
            .with_attribute("value", ScalarType::Float);
        let mut embeddings = Relation::new(RelationType::new(emb_type));

        // Doc 1: [1.0, 2.0]
        embeddings
            .insert(tuple! { doc_id: 1i64, dim: 0i64, value: 1.0f64 })
            .unwrap();
        embeddings
            .insert(tuple! { doc_id: 1i64, dim: 1i64, value: 2.0f64 })
            .unwrap();

        // Doc 2: [-1.0, 0.5]
        embeddings
            .insert(tuple! { doc_id: 2i64, dim: 0i64, value: -1.0f64 })
            .unwrap();
        embeddings
            .insert(tuple! { doc_id: 2i64, dim: 1i64, value: 0.5f64 })
            .unwrap();

        embeddings
    }

    #[test]
    fn test_dot_product_search() {
        let embeddings = setup_embeddings();
        let db = VectorDatabase::new(embeddings);

        let query_type = TupleType::new()
            .with_attribute("dim", ScalarType::Int)
            .with_attribute("query_value", ScalarType::Float);
        let mut query = Relation::new(RelationType::new(query_type));
        // Query: [1.0, 1.0]
        query
            .insert(tuple! { dim: 0i64, query_value: 1.0f64 })
            .unwrap();
        query
            .insert(tuple! { dim: 1i64, query_value: 1.0f64 })
            .unwrap();

        let results = db.search_dot_product(&query).unwrap();

        assert_eq!(results.cardinality(), 2);

        let mut doc1_score = 0.0;
        let mut doc2_score = 0.0;

        for t in results.tuples() {
            let doc_id = t.get_typed::<i64>("doc_id").unwrap();
            let score = t.get_typed::<f64>("score").unwrap();
            if doc_id == 1 {
                doc1_score = score;
            }
            if doc_id == 2 {
                doc2_score = score;
            }
        }

        // Doc 1: 1.0*1.0 + 2.0*1.0 = 3.0
        // Doc 2: -1.0*1.0 + 0.5*1.0 = -0.5
        assert!((doc1_score - 3.0).abs() < f64::EPSILON);
        assert!((doc2_score - -0.5).abs() < f64::EPSILON);
    }
}
