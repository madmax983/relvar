//! Relational Recommender System (Collaborative Filtering)
//!
//! This module demonstrates how to implement a user-based collaborative filtering
//! recommender system using pure Relational Algebra.
//!
//! # Concept
//!
//! A recommender system takes user-item interactions (e.g., ratings) and finds
//! similar users to recommend new items. This can be expressed relationally:
//!
//! 1.  **Similarity**: Find similar users by joining the ratings relation with itself
//!     on the item attribute. Users who rated the same items will form pairs. We
//!     calculate a similarity score (e.g., dot product) by multiplying their ratings
//!     and summarizing (grouping by user pairs).
//! 2.  **Candidates**: Join the similarity relation with the ratings of the other users
//!     to find items the target user hasn't seen yet.
//! 3.  **Prediction**: Compute a weighted score for each candidate item and rank them.

use relvar_core::algebra::{Aggregation, AggregationFn};
use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue};

/// A Relational Recommender System using User-Based Collaborative Filtering.
pub struct Recommender {
    /// The relation containing user-item ratings.
    /// Expected heading: (user_id: Int, item_id: Int, rating: Int)
    pub ratings: Relation,
    /// The name of the attribute representing the user ID.
    pub user_attr: String,
    /// The name of the attribute representing the item ID.
    pub item_attr: String,
    /// The name of the attribute representing the rating value.
    pub rating_attr: String,
}

impl Recommender {
    /// Creates a new Recommender.
    pub fn new(ratings: Relation, user_attr: &str, item_attr: &str, rating_attr: &str) -> Self {
        Self {
            ratings,
            user_attr: user_attr.to_string(),
            item_attr: item_attr.to_string(),
            rating_attr: rating_attr.to_string(),
        }
    }

    /// Computes user-user similarity matrix.
    /// Returns a relation with heading (u1: Int, u2: Int, similarity: Int)
    pub fn compute_similarities(&self) -> Result<Relation, DatabaseError> {
        // R1: (u1, item, r1)
        let r1_mappings = vec![
            (self.user_attr.as_str(), "u1"),
            (self.rating_attr.as_str(), "r1"),
        ];
        let r1 = self.ratings.rename(&r1_mappings);

        // R2: (u2, item, r2)
        let r2_mappings = vec![
            (self.user_attr.as_str(), "u2"),
            (self.rating_attr.as_str(), "r2"),
        ];
        let r2 = self.ratings.rename(&r2_mappings);

        // Join on item
        let joined = r1.join(&r2)?;

        // Restrict to u1 != u2
        let distinct_pairs = joined.restrict(|t| {
            let u1 = t.get_typed::<i64>("u1").unwrap();
            let u2 = t.get_typed::<i64>("u2").unwrap();
            u1 != u2
        });

        // Extend with product = r1 * r2
        let with_product = distinct_pairs
            .extend("product", ScalarType::Int, |t| {
                let r1 = t.get_typed::<i64>("r1").unwrap();
                let r2 = t.get_typed::<i64>("r2").unwrap();
                ScalarValue::Int(r1 * r2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize by u1, u2 to get dot product similarity
        let aggs = vec![Aggregation {
            result_name: "similarity".to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Sum("product".to_string()),
        }];

        let similarity = with_product
            .summarize(&["u1", "u2"], &aggs)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(similarity)
    }

    /// Recommends items for a target user.
    /// Returns a relation of (item_id: Int, score: Int)
    pub fn recommend(&self, target_user: i64) -> Result<Relation, DatabaseError> {
        let similarities = self.compute_similarities()?;

        // Restrict to target user's similarities: u1 == target_user
        let target_sims =
            similarities.restrict(move |t| t.get_typed::<i64>("u1").unwrap() == target_user);

        // Join with ratings of other users (u2 = user_id)
        // target_sims has (u1, u2, similarity). rename u2 -> user_id to join with self.ratings
        let target_sims_renamed = target_sims.rename(&[("u2", self.user_attr.as_str())]);

        // Joined has (u1, user_id, similarity, item_id, rating)
        let joined = target_sims_renamed.join(&self.ratings)?;

        // Find items the target user has already rated
        let target_rated_items = self
            .ratings
            .restrict(move |t| t.get_typed::<i64>(&self.user_attr).unwrap() == target_user)
            .project(&[&self.item_attr]);

        // Filter out items the user has already rated (Semidifference)
        // We only want items not in target_rated_items
        let candidate_items = joined.semidifference(&target_rated_items);

        // Extend with weighted score (similarity * rating)
        let with_score = candidate_items
            .extend("weighted_score", ScalarType::Int, |t| {
                let sim = t.get_typed::<i64>("similarity").unwrap();
                let rating = t.get_typed::<i64>(&self.rating_attr).unwrap();
                ScalarValue::Int(sim * rating)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize by item_id to get total score
        let aggs = vec![Aggregation {
            result_name: "score".to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Sum("weighted_score".to_string()),
        }];

        let recommendations = with_score
            .summarize(&[&self.item_attr], &aggs)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Return (item_id, score)
        Ok(recommendations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    #[test]
    fn test_recommender() {
        // Schema: (user: Int, item: Int, rating: Int)
        let heading = TupleType::new()
            .with_attribute("user", ScalarType::Int)
            .with_attribute("item", ScalarType::Int)
            .with_attribute("rating", ScalarType::Int);

        let mut ratings = Relation::new(RelationType::new(heading));

        // User 1 likes items 1 and 2
        ratings
            .insert(tuple! { user: 1i64, item: 1i64, rating: 5i64 })
            .unwrap();
        ratings
            .insert(tuple! { user: 1i64, item: 2i64, rating: 4i64 })
            .unwrap();

        // User 2 likes items 1, 2, and 3
        ratings
            .insert(tuple! { user: 2i64, item: 1i64, rating: 4i64 })
            .unwrap();
        ratings
            .insert(tuple! { user: 2i64, item: 2i64, rating: 5i64 })
            .unwrap();
        ratings
            .insert(tuple! { user: 2i64, item: 3i64, rating: 5i64 })
            .unwrap();

        // User 3 likes items 3 and 4 (different tastes)
        ratings
            .insert(tuple! { user: 3i64, item: 3i64, rating: 5i64 })
            .unwrap();
        ratings
            .insert(tuple! { user: 3i64, item: 4i64, rating: 5i64 })
            .unwrap();

        let recommender = Recommender::new(ratings, "user", "item", "rating");

        let recs = recommender.recommend(1).unwrap();

        // User 1 should be recommended item 3 (because of user 2) but NOT item 4 (user 3 isn't similar)
        // Let's check the result
        // User 1 and User 2 similarity: (5*4) + (4*5) = 20 + 20 = 40
        // User 1 and User 3 similarity: 0 (no items in common)
        // User 2 rating for item 3 is 5.
        // Weighted score for item 3 = 40 * 5 = 200.

        assert!(recs.cardinality() > 0);
        let t = recs
            .into_iter()
            .find(|t| t.get_typed::<i64>("item").unwrap() == 3)
            .unwrap();
        assert_eq!(t.get_typed::<i64>("score").unwrap(), 200);
    }
}
