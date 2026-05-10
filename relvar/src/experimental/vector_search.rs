//! Relational Vector Search
//!
//! This module demonstrates how vector similarity search can be implemented
//! using purely relational algebra. It represents vectors as a relation of
//! dimensions and values, and computes similarity scores (e.g., dot product)
//! via Join, Extend, and Summarize operations.
//!
//! # Concept
//!
//! We represent a collection of vectors as a relation:
//! `(id: Int, dim: Int, value: Float)`.
//!
//! A target query vector is represented as:
//! `(dim: Int, target_value: Float)`.
//!
//! By joining the two relations on `dim`, we pair corresponding components.
//! We then use `Extend` to compute the product of components (for dot product)
//! or squared difference (for L2 distance). Finally, we use `Summarize` grouping
//! by `id` and summing the components to compute the final similarity score or distance.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Vector Database.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::vector_search::VectorDatabase;
/// // Note: This is a placeholder example
/// ```
pub struct VectorDatabase {
    /// The relation storing the vectors.
    /// Schema: (id: Int, dim: Int, value: Float)
    pub vectors: Relation,
}

impl VectorDatabase {
    /// Creates a new VectorDatabase from an existing relation of vectors.
    ///
    /// `vectors` must have the schema `(id: Int, dim: Int, value: Float)`.
    pub fn new(vectors: Relation) -> Self {
        Self { vectors }
    }

    /// Computes the dot product similarity between the database vectors and a target vector.
    ///
    /// The `target_vector` must be a relation with the schema `(dim: Int, target_value: Float)`.
    ///
    /// Returns a relation with the schema `(id: Int, score: Float)`, where higher scores
    /// indicate greater similarity.
    pub fn dot_product_search(&self, target_vector: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Join on `dim`
        // Result schema: (id: Int, dim: Int, value: Float, target_value: Float)
        let joined = self.vectors.join(target_vector)?;

        // 2. Compute component-wise product
        // Result schema: (id: Int, dim: Int, value: Float, target_value: Float, product: Float)
        let extended = joined
            .extend("product", ScalarType::Float, |t| {
                let v = t.get_typed::<f64>("value").unwrap_or(0.0);
                let tv = t.get_typed::<f64>("target_value").unwrap_or(0.0);
                ScalarValue::Float(v * tv)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by `id`, summing the products to compute the dot product score
        // Result schema: (id: Int, score: Float)
        let scores = extended
            .summarize(&["id"], &[Aggregation::sum_float("score", "product")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(scores)
    }

    /// Computes the squared L2 distance between the database vectors and a target vector.
    ///
    /// The `target_vector` must be a relation with the schema `(dim: Int, target_value: Float)`.
    ///
    /// Returns a relation with the schema `(id: Int, distance: Float)`, where lower distances
    /// indicate greater similarity.
    pub fn l2_distance_search(&self, target_vector: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Join on `dim`
        let joined = self.vectors.join(target_vector)?;

        // 2. Compute component-wise squared difference
        let extended = joined
            .extend("sq_diff", ScalarType::Float, |t| {
                let v = t.get_typed::<f64>("value").unwrap_or(0.0);
                let tv = t.get_typed::<f64>("target_value").unwrap_or(0.0);
                let diff = v - tv;
                ScalarValue::Float(diff * diff)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by `id`, summing the squared differences to compute the squared L2 distance
        let distances = extended
            .summarize(&["id"], &[Aggregation::sum_float("distance", "sq_diff")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(distances)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    fn create_test_database() -> VectorDatabase {
        let vectors_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("dim", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        );
        let mut vectors = Relation::new(vectors_type);

        // Vector 1: [1.0, 0.0]
        vectors
            .insert(tuple! { id: 1i64, dim: 0i64, value: 1.0f64 })
            .unwrap();
        vectors
            .insert(tuple! { id: 1i64, dim: 1i64, value: 0.0f64 })
            .unwrap();

        // Vector 2: [0.0, 1.0]
        vectors
            .insert(tuple! { id: 2i64, dim: 0i64, value: 0.0f64 })
            .unwrap();
        vectors
            .insert(tuple! { id: 2i64, dim: 1i64, value: 1.0f64 })
            .unwrap();

        // Vector 3: [1.0, 1.0]
        vectors
            .insert(tuple! { id: 3i64, dim: 0i64, value: 1.0f64 })
            .unwrap();
        vectors
            .insert(tuple! { id: 3i64, dim: 1i64, value: 1.0f64 })
            .unwrap();

        VectorDatabase::new(vectors)
    }

    fn create_target_vector(v0: f64, v1: f64) -> Relation {
        let target_type = RelationType::new(
            TupleType::new()
                .with_attribute("dim", ScalarType::Int)
                .with_attribute("target_value", ScalarType::Float),
        );
        let mut target = Relation::new(target_type);
        target
            .insert(tuple! { dim: 0i64, target_value: v0 })
            .unwrap();
        target
            .insert(tuple! { dim: 1i64, target_value: v1 })
            .unwrap();
        target
    }

    #[test]
    fn test_dot_product_search() {
        let db = create_test_database();
        // Target: [1.0, 0.0]
        let target = create_target_vector(1.0, 0.0);

        let scores = db.dot_product_search(&target).unwrap();

        // Check scores: V1: 1.0, V2: 0.0, V3: 1.0
        let mut score_map = std::collections::HashMap::new();
        for t in scores.tuples() {
            let id = t.get_typed::<i64>("id").unwrap();
            let score = t.get_typed::<f64>("score").unwrap();
            score_map.insert(id, score);
        }

        assert_eq!(score_map.get(&1), Some(&1.0));
        assert_eq!(score_map.get(&2), Some(&0.0));
        assert_eq!(score_map.get(&3), Some(&1.0));
    }

    #[test]
    fn test_l2_distance_search() {
        let db = create_test_database();
        // Target: [1.0, 0.0]
        let target = create_target_vector(1.0, 0.0);

        let distances = db.l2_distance_search(&target).unwrap();

        // Check L2 distances squared:
        // V1: (1-1)^2 + (0-0)^2 = 0
        // V2: (0-1)^2 + (1-0)^2 = 2
        // V3: (1-1)^2 + (1-0)^2 = 1
        let mut dist_map = std::collections::HashMap::new();
        for t in distances.tuples() {
            let id = t.get_typed::<i64>("id").unwrap();
            let dist = t.get_typed::<f64>("distance").unwrap();
            dist_map.insert(id, dist);
        }

        assert_eq!(dist_map.get(&1), Some(&0.0));
        assert_eq!(dist_map.get(&2), Some(&2.0));
        assert_eq!(dist_map.get(&3), Some(&1.0));
    }
}
