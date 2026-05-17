//! Relational K-Nearest Neighbors (KNN)
//!
//! This module implements the K-Nearest Neighbors classification algorithm
//! using purely relational algebra. It computes distances, ranks neighbors
//! using a declarative Top-N pattern (via Cartesian Product and Aggregation),
//! and determines the majority class via Grouping and Summarization.
//!
//! # Concept
//!
//! 1. **Distance**: Compute Euclidean distance between the query point and training data via `Extend`.
//! 2. **Ranking (Top-K)**: Compute the rank of each neighbor by joining distances with themselves where `d1 > d2`, summarizing the count.
//! 3. **Voting**: Restrict to `rank < K`, summarize by class to count votes, and use another Top-1 pattern to find the majority class.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational K-Nearest Neighbors Classifier.
pub struct KnnClassifier {
    /// The training data.
    /// Schema must include at least: (id: Int, x: Float, y: Float, class: String)
    pub training_data: Relation,
}

impl KnnClassifier {
    /// Creates a new KnnClassifier.
    pub fn new(training_data: Relation) -> Self {
        Self { training_data }
    }

    /// Predicts the class for a given query point.
    /// `k` is the number of neighbors to consider.
    pub fn predict(&self, query_x: f64, query_y: f64, k: i64) -> Result<String, DatabaseError> {
        // 1. Compute distances
        let distances = self
            .training_data
            .extend("distance", ScalarType::Float, move |t| {
                let x = t.get_typed::<f64>("x").unwrap();
                let y = t.get_typed::<f64>("y").unwrap();
                let dx = x - query_x;
                let dy = y - query_y;
                ScalarValue::Float((dx * dx + dy * dy).sqrt())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Rank the neighbors (Top-K pattern)
        let d1 = distances.clone().rename(&[
            ("id", "id1"),
            ("x", "x1"),
            ("y", "y1"),
            ("class", "class1"),
            ("distance", "dist1"),
        ]);

        let d2 = distances.clone().rename(&[
            ("id", "id2"),
            ("x", "x2"),
            ("y", "y2"),
            ("class", "class2"),
            ("distance", "dist2"),
        ]);

        let cross = d1.join(&d2)?;

        let strictly_smaller = cross.restrict(|t| {
            let dist1 = t.get_typed::<f64>("dist1").unwrap();
            let dist2 = t.get_typed::<f64>("dist2").unwrap();
            if dist2 < dist1 {
                true
            } else if (dist2 - dist1).abs() < f64::EPSILON {
                let id1 = t.get_typed::<i64>("id1").unwrap();
                let id2 = t.get_typed::<i64>("id2").unwrap();
                id2 < id1
            } else {
                false
            }
        });

        let ranks = strictly_smaller
            .summarize(&["id1"], &[Aggregation::count("rank")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let d1_ids = d1.project(&["id1"]);
        let ranked_ids = ranks.project(&["id1"]);
        let unranked_ids = d1_ids
            .difference(&ranked_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let unranked_with_zero = unranked_ids
            .extend("rank", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_ranks = ranks
            .union(&unranked_with_zero)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let ranked_neighbors = d1.join(&all_ranks)?;

        // 3. Filter to Top-K
        let top_k = ranked_neighbors.restrict(move |t| {
            let rank = t.get_typed::<i64>("rank").unwrap();
            rank < k
        });

        // 4. Vote!
        let votes = top_k
            .summarize(&["class1"], &[Aggregation::count("votes")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Find the class with the maximum votes (Top-1 pattern)
        let v1 = votes.clone().rename(&[("class1", "c1"), ("votes", "v1")]);
        let v2 = votes.clone().rename(&[("class1", "c2"), ("votes", "v2")]);

        let v_cross = v1.join(&v2)?;

        let smaller_votes = v_cross.restrict(|t| {
            let v_1 = t.get_typed::<i64>("v1").unwrap();
            let v_2 = t.get_typed::<i64>("v2").unwrap();
            if v_2 > v_1 {
                true
            } else if v_2 == v_1 {
                let c_1 = t.get_typed::<String>("c1").unwrap();
                let c_2 = t.get_typed::<String>("c2").unwrap();
                c_2 < c_1
            } else {
                false
            }
        });

        let vote_ranks = smaller_votes
            .summarize(&["c1"], &[Aggregation::count("vote_rank")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let v1_ids = v1.project(&["c1"]);
        let ranked_v_ids = vote_ranks.project(&["c1"]);
        let unranked_v_ids = v1_ids
            .difference(&ranked_v_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let winner_tuple = unranked_v_ids
            .tuples()
            .next()
            .ok_or_else(|| DatabaseError::AlgebraError("No winner found".into()))?;

        Ok(winner_tuple.get_typed::<String>("c1").unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_knn() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float)
            .with_attribute("class", ScalarType::String);

        let mut training = Relation::new(RelationType::new(heading));

        training
            .insert(tuple! { id: 1i64, x: 0.0f64, y: 0.1f64, class: "A".to_string() })
            .unwrap();
        training
            .insert(tuple! { id: 2i64, x: 0.1f64, y: 0.0f64, class: "A".to_string() })
            .unwrap();
        training
            .insert(tuple! { id: 3i64, x: -0.1f64, y: 0.0f64, class: "A".to_string() })
            .unwrap();

        training
            .insert(tuple! { id: 4i64, x: 10.0f64, y: 10.1f64, class: "B".to_string() })
            .unwrap();
        training
            .insert(tuple! { id: 5i64, x: 10.1f64, y: 10.0f64, class: "B".to_string() })
            .unwrap();
        training
            .insert(tuple! { id: 6i64, x: 9.9f64, y: 10.0f64, class: "B".to_string() })
            .unwrap();

        let knn = KnnClassifier::new(training);

        let res_a = knn.predict(0.0, 0.0, 3).unwrap();
        assert_eq!(res_a, "A");

        let res_b = knn.predict(10.0, 10.0, 3).unwrap();
        assert_eq!(res_b, "B");
    }
}
