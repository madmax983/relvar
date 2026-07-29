//! Relational Vector Database Simulator.
//!
//! Models a vector database computing Cosine Similarity for dense embeddings
//! using purely relational algebra (Joins, Extensions, Summarizations).

use relvar_core::algebra::Aggregation;
use relvar_core::{DatabaseError, Relation, ScalarType, ScalarValue};

/// A simple Relational Vector Database
#[allow(dead_code)]
pub struct VectorDB {
    /// Relation containing embeddings: `(item_id: String, dim: Int, value: Float)`
    pub embeddings: Relation,
}

impl VectorDB {
    /// Creates a new VectorDB with the given embeddings relation.
    pub fn new(embeddings: Relation) -> Self {
        Self { embeddings }
    }

    /// Computes the cosine similarity between the embeddings in the database and a given query.
    ///
    /// `query` is a relation with schema `(dim: Int, value: Float)`.
    /// Returns a relation with schema `(item_id: String, similarity: Float)`.
    pub fn search(&self, query: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Compute dot product
        // Embeddings: (item_id, dim, value)
        // Query: (dim, query_value)
        let q_renamed = query.clone().rename_into(&[("value", "query_value")]);
        let joined = self
            .embeddings
            .clone()
            .join(&q_renamed)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let multiplied = joined
            .extend("prod", ScalarType::Float, |t| {
                let v = t.get_typed::<f64>("value").unwrap_or(0.0);
                let qv = t.get_typed::<f64>("query_value").unwrap_or(0.0);
                ScalarValue::Float(v * qv)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let dot_product = multiplied
            .summarize(
                &["item_id"],
                &[Aggregation::sum_float("dot_product", "prod")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Compute query magnitude
        let q_sq = query
            .clone()
            .extend("sq", ScalarType::Float, |t| {
                let v = t.get_typed::<f64>("value").unwrap_or(0.0);
                ScalarValue::Float(v * v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let q_mag_rel = q_sq
            .summarize(&[], &[Aggregation::sum_float("q_sq_sum", "sq")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Compute item magnitudes
        let item_sq = self
            .embeddings
            .clone()
            .extend("sq", ScalarType::Float, |t| {
                let v = t.get_typed::<f64>("value").unwrap_or(0.0);
                ScalarValue::Float(v * v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let item_mag_rel = item_sq
            .summarize(&["item_id"], &[Aggregation::sum_float("item_sq_sum", "sq")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Combine and compute final similarity
        let combined = dot_product
            .join(&item_mag_rel)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        // Cross join with query magnitude
        let fully_combined = combined
            .join(&q_mag_rel)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let similarity = fully_combined
            .extend("similarity", ScalarType::Float, |t| {
                let dot = t.get_typed::<f64>("dot_product").unwrap_or(0.0);
                let q_sq_sum = t.get_typed::<f64>("q_sq_sum").unwrap_or(0.0);
                let item_sq_sum = t.get_typed::<f64>("item_sq_sum").unwrap_or(0.0);

                let mag = (q_sq_sum * item_sq_sum).sqrt();
                let sim = if mag == 0.0 { 0.0 } else { dot / mag };
                ScalarValue::Float(sim)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let result = similarity.project_into(&["item_id", "similarity"]);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{RelationType, TupleType, tuple};

    #[test]
    fn test_vector_search() -> Result<(), DatabaseError> {
        let emb_type = RelationType::new(
            TupleType::new()
                .with_attribute("item_id", ScalarType::String)
                .with_attribute("dim", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        );
        let mut embeddings = Relation::new(emb_type);

        // Item A: [1.0, 0.0]
        let _ = embeddings
            .insert(tuple! { item_id: "A".to_string(), dim: 0i64, value: 1.0f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;
        let _ = embeddings
            .insert(tuple! { item_id: "A".to_string(), dim: 1i64, value: 0.0f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;

        // Item B: [0.0, 1.0]
        let _ = embeddings
            .insert(tuple! { item_id: "B".to_string(), dim: 0i64, value: 0.0f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;
        let _ = embeddings
            .insert(tuple! { item_id: "B".to_string(), dim: 1i64, value: 1.0f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;

        // Item C: [0.707, 0.707]
        let _ = embeddings
            .insert(tuple! { item_id: "C".to_string(), dim: 0i64, value: 0.707f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;
        let _ = embeddings
            .insert(tuple! { item_id: "C".to_string(), dim: 1i64, value: 0.707f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;

        let db = VectorDB::new(embeddings);

        // Query: [1.0, 0.0]
        let query_type = RelationType::new(
            TupleType::new()
                .with_attribute("dim", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        );
        let mut query = Relation::new(query_type);
        let _ = query
            .insert(tuple! { dim: 0i64, value: 1.0f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;
        let _ = query
            .insert(tuple! { dim: 1i64, value: 0.0f64 })
            .map_err(|_| DatabaseError::AlgebraError("insert fail".to_string()))?;

        let results = db.search(&query)?;

        assert_eq!(results.cardinality(), 3);

        // Find similarities
        let mut sim_a = 0.0_f64;
        let mut sim_b = 0.0_f64;
        let mut sim_c = 0.0_f64;

        for t in results.tuples() {
            let id = t.get_typed::<String>("item_id").unwrap_or_default();
            let sim = t.get_typed::<f64>("similarity").unwrap_or_default();
            match id.as_str() {
                "A" => sim_a = sim,
                "B" => sim_b = sim,
                "C" => sim_c = sim,
                _ => panic!("Unknown item_id"),
            }
        }

        assert!((sim_a - 1.0).abs() < 0.001);
        assert!((sim_b - 0.0).abs() < 0.001);
        assert!((sim_c - 0.707).abs() < 0.001);

        Ok(())
    }
}
