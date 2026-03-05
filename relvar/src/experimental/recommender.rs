//! Experimental Relational Recommender System.
//!
//! This module demonstrates how to implement a collaborative filtering recommendation
//! system using pure Relational Algebra primitives.
//!
//! # Philosophy
//!
//! Collaborative filtering algorithms typically rely on linear algebra (matrix factorization)
//! or imperative loops to calculate similarities and predictions. By modeling the problem
//! as a bipartite graph of `Users` and `Items` connected by `Ratings`, we can express
//! these algorithms entirely through relational operators:
//!
//! - **Similarity Calculation (e.g., Cosine Similarity):**
//!   Implemented via Self-Join (to find co-rated items), Extend (to calculate dot products
//!   and vector magnitudes), and Summarize (to sum components per user pair).
//! - **Prediction & Recommendation:**
//!   Implemented via Join (users with similar users' ratings), Extend (to weight ratings by similarity),
//!   Semidifference (to exclude items the user has already rated), and Summarize (to aggregate
//!   and normalize the predicted score).

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// Helper to convert a numeric rating to `f64`.
fn get_rating_as_f64(tuple: &Tuple, attr_name: &str) -> f64 {
    match tuple.get(attr_name) {
        Some(ScalarValue::Int(i)) => *i as f64,
        Some(ScalarValue::Float(f)) => *f,
        _ => panic!("Rating must be a numeric type"),
    }
}

/// Helper to get an `i64` from a tuple.
#[cfg(test)]
fn get_i64(tuple: &Tuple, attr_name: &str) -> i64 {
    match tuple.get(attr_name) {
        Some(ScalarValue::Int(i)) => *i,
        _ => panic!("Expected Int attribute"),
    }
}

/// Helper to get an `f64` from a tuple.
fn get_f64(tuple: &Tuple, attr_name: &str) -> f64 {
    match tuple.get(attr_name) {
        Some(ScalarValue::Float(f)) => *f,
        Some(ScalarValue::Int(i)) => *i as f64,
        _ => panic!("Expected Float attribute"),
    }
}

/// A Relational Recommender System.
pub struct Recommender {
    user_id_attr: String,
    item_id_attr: String,
    rating_attr: String,
}

impl Recommender {
    /// Creates a new Recommender instance.
    ///
    /// # Arguments
    ///
    /// * `user_id_attr` - The name of the user ID attribute.
    /// * `item_id_attr` - The name of the item ID attribute.
    /// * `rating_attr` - The name of the rating attribute (must be Float or Int).
    pub fn new(user_id_attr: &str, item_id_attr: &str, rating_attr: &str) -> Self {
        Self {
            user_id_attr: user_id_attr.to_string(),
            item_id_attr: item_id_attr.to_string(),
            rating_attr: rating_attr.to_string(),
        }
    }

    /// Computes user-user cosine similarities based on their item ratings.
    ///
    /// # Arguments
    ///
    /// * `ratings` - The relation of ratings (e.g., `(user_id, item_id, rating)`).
    ///
    /// # Returns
    ///
    /// A relation with heading `(user_id_1, user_id_2, similarity)` containing similarity scores.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if internal operations fail.
    pub fn compute_similarities(&self, ratings: &Relation) -> Result<Relation, DatabaseError> {
        let u1 = "u1";
        let u2 = "u2";
        let r1 = "r1";
        let r2 = "r2";

        // 1. Prepare Self-Join (Find co-rated items)
        let rename_map_1 = vec![
            (self.user_id_attr.as_str(), u1),
            (self.rating_attr.as_str(), r1),
        ];
        let rename_map_2 = vec![
            (self.user_id_attr.as_str(), u2),
            (self.rating_attr.as_str(), r2),
        ];

        let r1_rel = ratings.rename(&rename_map_1);
        let r2_rel = ratings.rename(&rename_map_2);

        // Natural Join on item_id to find co-rated items
        let co_rated = r1_rel.join(&r2_rel)?;

        // Exclude self-similarities (u1 == u2)
        let pairs = co_rated.restrict(move |t| {
            let user1 = t.get(u1).unwrap();
            let user2 = t.get(u2).unwrap();
            user1 != user2
        });

        // 2. Extend with partial computations for Cosine Similarity
        // dot_product = r1 * r2
        // r1_sq = r1 * r1
        // r2_sq = r2 * r2
        let extended_pairs = pairs
            .extend("dot_product", ScalarType::Float, move |t| {
                let rating1 = get_rating_as_f64(t, r1);
                let rating2 = get_rating_as_f64(t, r2);
                ScalarValue::Float(rating1 * rating2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("r1_sq", ScalarType::Float, move |t| {
                let rating1 = get_rating_as_f64(t, r1);
                ScalarValue::Float(rating1 * rating1)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("r2_sq", ScalarType::Float, move |t| {
                let rating2 = get_rating_as_f64(t, r2);
                ScalarValue::Float(rating2 * rating2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize (Group By u1, u2)
        // sum(dot_product), sum(r1_sq), sum(r2_sq)
        let summarized_pairs = extended_pairs
            .summarize(
                &[u1, u2],
                &[
                    Aggregation::sum_float("sum_dot", "dot_product"),
                    Aggregation::sum_float("sum_r1_sq", "r1_sq"),
                    Aggregation::sum_float("sum_r2_sq", "r2_sq"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Calculate Final Cosine Similarity
        // similarity = sum_dot / (sqrt(sum_r1_sq) * sqrt(sum_r2_sq))
        let similarities = summarized_pairs
            .extend("similarity", ScalarType::Float, move |t| {
                let sum_dot = get_f64(t, "sum_dot");
                let sum_r1_sq = get_f64(t, "sum_r1_sq");
                let sum_r2_sq = get_f64(t, "sum_r2_sq");

                let denominator = sum_r1_sq.sqrt() * sum_r2_sq.sqrt();
                let sim = if denominator == 0.0 {
                    0.0
                } else {
                    sum_dot / denominator
                };

                ScalarValue::Float(sim)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Project and Rename to final schema: (user_id_1, user_id_2, similarity)
        let result = similarities
            .project(&[u1, u2, "similarity"])
            .rename(&[(u1, "user_id_1"), (u2, "user_id_2")]);

        Ok(result)
    }

    /// Computes recommendations for all users.
    ///
    /// # Arguments
    ///
    /// * `ratings` - The original relation of user-item ratings.
    /// * `similarities` - The relation of user similarities computed by `compute_similarities`.
    ///
    /// # Returns
    ///
    /// A relation with heading `(user_id, item_id, predicted_rating)`.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if operations fail.
    pub fn recommend(
        &self,
        ratings: &Relation,
        similarities: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 1. Join Similarities with Ratings
        // similarities: (user_id_1, user_id_2, similarity)
        // ratings: (user_id, item_id, rating)
        // We want: (user_id_1, user_id_2, similarity, item_id, rating) where user_id_2 = user_id
        let r_renamed = ratings.rename(&[
            (self.user_id_attr.as_str(), "user_id_2"),
            (self.rating_attr.as_str(), "rating_2"),
        ]);
        let sim_ratings = similarities.join(&r_renamed)?;

        // Filter out negative or zero similarities if desired, here we just keep positive ones
        let positive_sims = sim_ratings.restrict(|t| {
            if let Some(ScalarValue::Float(sim)) = t.get("similarity") {
                *sim > 0.0
            } else {
                false
            }
        });

        // 2. Semidifference: Remove items the user has already rated
        // We need the user's rated items
        // ratings: (user_id, item_id) -> rename to (user_id_1, item_id)
        let user_items = ratings
            .project(&[self.user_id_attr.as_str(), self.item_id_attr.as_str()])
            .rename(&[(self.user_id_attr.as_str(), "user_id_1")]);

        // Semidifference: positive_sims minus user_items
        let unrated_items = positive_sims.semidifference(&user_items);

        // 3. Extend: Weighted Rating
        // weighted_rating = similarity * rating_2
        let weighted_unrated = unrated_items
            .extend("weighted_rating", ScalarType::Float, move |t| {
                let sim = get_rating_as_f64(t, "similarity");
                let rating = get_rating_as_f64(t, "rating_2");
                ScalarValue::Float(sim * rating)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize: Predict Rating
        // Group by (user_id_1, item_id)
        // predicted_rating = sum(weighted_rating) / sum(similarity)
        let summarized_predictions = weighted_unrated
            .summarize(
                &["user_id_1", self.item_id_attr.as_str()],
                &[
                    Aggregation::sum_float("sum_weighted_rating", "weighted_rating"),
                    Aggregation::sum_float("sum_similarity", "similarity"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let predictions = summarized_predictions
            .extend("predicted_rating", ScalarType::Float, move |t| {
                let sum_weighted = get_f64(t, "sum_weighted_rating");
                let sum_sim = get_f64(t, "sum_similarity");

                let pred = if sum_sim == 0.0 {
                    0.0
                } else {
                    sum_weighted / sum_sim
                };

                ScalarValue::Float(pred)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Finalize Schema
        let result = predictions
            .project(&["user_id_1", self.item_id_attr.as_str(), "predicted_rating"])
            .rename(&[("user_id_1", self.user_id_attr.as_str())]);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::ScalarType;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_recommender() -> Result<(), DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());
        let recommender = Recommender::new("user_id", "item_id", "rating");

        let heading = TupleType::new()
            .with_attribute("user_id", ScalarType::Int)
            .with_attribute("item_id", ScalarType::Int)
            .with_attribute("rating", ScalarType::Float);

        db.create_relvar("RATINGS", RelationType::new(heading))?;

        // User 1
        db.insert("RATINGS", tuple! { user_id: 1, item_id: 101, rating: 5.0 })?;
        db.insert("RATINGS", tuple! { user_id: 1, item_id: 102, rating: 4.0 })?;

        // User 2 (very similar to User 1)
        db.insert("RATINGS", tuple! { user_id: 2, item_id: 101, rating: 5.0 })?;
        db.insert("RATINGS", tuple! { user_id: 2, item_id: 102, rating: 4.0 })?;
        db.insert("RATINGS", tuple! { user_id: 2, item_id: 103, rating: 5.0 })?; // Unrated by User 1

        // User 3 (different from User 1)
        db.insert("RATINGS", tuple! { user_id: 3, item_id: 101, rating: 1.0 })?;
        db.insert("RATINGS", tuple! { user_id: 3, item_id: 104, rating: 5.0 })?;

        let ratings = db.query("RATINGS")?;

        let similarities = recommender.compute_similarities(&ratings)?;

        let recs = recommender.recommend(&ratings, &similarities)?;

        let u1_recs = recs.restrict(|t| get_i64(t, "user_id") == 1);

        let mut found_103 = false;
        let mut found_104 = false;

        for t in u1_recs.tuples() {
            let item_id = get_i64(t, "item_id");
            let rating = get_f64(t, "predicted_rating");

            if item_id == 103 {
                found_103 = true;
                assert!(rating > 4.5); // From User 2 (high sim)
            } else if item_id == 104 {
                found_104 = true;
                assert!(rating > 0.0); // From User 3 (lower sim)
            }
        }

        assert!(found_103, "Should recommend 103");
        assert!(found_104, "Should recommend 104");

        Ok(())
    }
}
