//! Relational Collaborative Filtering Recommender System
//!
//! This module implements a User-User Collaborative Filtering recommender
//! system using purely relational algebra.
//!
//! # Concept
//!
//! Predictions are modeled via relational operations:
//! - **Self-Joins**: To find users who rated the same items.
//! - **Extensions**: To calculate rating products and weighted scores.
//! - **Summarizations**: To compute similarity scores and final predictions.
//! - **Semidifferences / Differences**: To find items the target user hasn't rated yet.
//!
//! # Example
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::experimental::recommender::Recommender;
//! use relvar::{RelationType, ScalarType, TupleType};
//! use relvar::{ScalarValue, Relation};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Setup Database
//! let mut db = Database::new(InMemoryEngine::new());
//!
//! // 2. Create Ratings Data
//! let heading = TupleType::new()
//!     .with_attribute("user", ScalarType::String)
//!     .with_attribute("item", ScalarType::String)
//!     .with_attribute("rating", ScalarType::Float);
//!
//! let mut ratings = Relation::new(RelationType::new(heading));
//! ratings.insert(tuple! { user: "Alice", item: "Matrix", rating: 5.0 })?;
//! ratings.insert(tuple! { user: "Alice", item: "Inception", rating: 4.0 })?;
//! ratings.insert(tuple! { user: "Bob", item: "Matrix", rating: 4.0 })?;
//! ratings.insert(tuple! { user: "Bob", item: "Avatar", rating: 5.0 })?;
//!
//! // 3. Compute Recommendations for Alice
//! let recs = Recommender::recommend(&ratings, ScalarValue::String("Alice".to_string()), "user", "item", "rating")?;
//!
//! // Alice hasn't seen Avatar, Bob rated it 5.0 and they share "Matrix".
//! // So Avatar should be recommended.
//! assert!(recs.cardinality() > 0);
//! # Ok(())
//! # }
//! ```

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue};

/// A Relational Collaborative Filtering Recommender System.
pub struct Recommender;

impl Recommender {
    /// Generates recommendations for a target user based on user-user collaborative filtering.
    ///
    /// The algorithm:
    /// 1. Finds the target user's ratings.
    /// 2. Finds other users' ratings.
    /// 3. Joins on `item_attr` to find co-rated items.
    /// 4. Computes a similarity score (dot product of ratings).
    /// 5. Finds items the target user HAS NOT rated.
    /// 6. Computes weighted predictions for those unseen items.
    pub fn recommend(
        ratings: &Relation,
        target_user: ScalarValue,
        user_attr: &str,
        item_attr: &str,
        rating_attr: &str,
    ) -> Result<Relation, DatabaseError> {
        // 1. Target user's ratings
        let target_val = target_user.clone();
        let target_ratings = ratings.restrict(move |t| t.get(user_attr) == Some(&target_val));

        // 2. Other users' ratings
        let target_val2 = target_user.clone();
        let other_ratings = ratings.restrict(move |t| t.get(user_attr) != Some(&target_val2));

        // 3. To join them, we need to rename rating and user attributes to avoid conflicts
        let target_renamed =
            target_ratings.rename(&[(user_attr, "t_user"), (rating_attr, "t_rating")]);

        let other_renamed =
            other_ratings.rename(&[(user_attr, "o_user"), (rating_attr, "o_rating")]);

        // Join on item_attr
        let corated = target_renamed.join(&other_renamed)?;

        // 4. Compute similarity (dot product of ratings)
        let sim_extended = corated
            .extend("product", ScalarType::Float, |t| {
                let tr = t.get_typed::<f64>("t_rating").unwrap();
                let or = t.get_typed::<f64>("o_rating").unwrap();
                ScalarValue::Float(tr * or)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize to get similarity per user
        let similarity = sim_extended
            .summarize(
                &["o_user"],
                &[Aggregation::sum_float("similarity", "product")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Find unseen items for the target user
        let all_items = ratings.project(&[item_attr]);
        let target_items = target_ratings.project(&[item_attr]);
        let unseen_items = all_items
            .difference(&target_items)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Get other users' ratings for these unseen items
        // Join unseen_items with other_ratings
        let unseen_ratings = unseen_items.join(&other_renamed)?;

        // 7. Weight these ratings by similarity
        // Join with similarity relation
        let joined_with_sim = unseen_ratings.join(&similarity)?;

        // Extend to calculate weighted rating: o_rating * similarity
        let weighted = joined_with_sim
            .extend("weighted_rating", ScalarType::Float, |t| {
                let or = t.get_typed::<f64>("o_rating").unwrap();
                let sim = t.get_typed::<f64>("similarity").unwrap();
                ScalarValue::Float(or * sim)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize by item to get prediction: sum(weighted_rating) / sum(similarity)
        // First, get the sums
        let item_sums = weighted
            .summarize(
                &[item_attr],
                &[
                    Aggregation::sum_float("sum_weighted", "weighted_rating"),
                    Aggregation::sum_float("sum_sim", "similarity"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Finally, calculate prediction
        let predictions = item_sums
            .extend("prediction", ScalarType::Float, |t| {
                let sw = t.get_typed::<f64>("sum_weighted").unwrap();
                let ss = t.get_typed::<f64>("sum_sim").unwrap();
                if ss == 0.0 {
                    ScalarValue::Float(0.0)
                } else {
                    ScalarValue::Float(sw / ss)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Project final result (item, prediction)
        Ok(predictions.project(&[item_attr, "prediction"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_recommender() {
        let heading = TupleType::new()
            .with_attribute("user", ScalarType::String)
            .with_attribute("item", ScalarType::String)
            .with_attribute("rating", ScalarType::Float);

        let mut ratings = Relation::new(RelationType::new(heading));
        ratings
            .insert(tuple! { user: "Alice", item: "Matrix", rating: 5.0 })
            .unwrap();
        ratings
            .insert(tuple! { user: "Alice", item: "Inception", rating: 4.0 })
            .unwrap();

        ratings
            .insert(tuple! { user: "Bob", item: "Matrix", rating: 4.0 })
            .unwrap();
        ratings
            .insert(tuple! { user: "Bob", item: "Avatar", rating: 5.0 })
            .unwrap();

        ratings
            .insert(tuple! { user: "Charlie", item: "Inception", rating: 5.0 })
            .unwrap();
        ratings
            .insert(tuple! { user: "Charlie", item: "Interstellar", rating: 4.0 })
            .unwrap();

        let recs = Recommender::recommend(
            &ratings,
            ScalarValue::String("Alice".to_string()),
            "user",
            "item",
            "rating",
        )
        .unwrap();

        // Alice should be recommended Avatar (from Bob) and Interstellar (from Charlie)
        assert_eq!(recs.cardinality(), 2);

        let avatar_rec = recs
            .tuples()
            .find(|t| t.get_typed::<String>("item").unwrap() == "Avatar")
            .unwrap();
        let avatar_pred = avatar_rec.get_typed::<f64>("prediction").unwrap();

        assert!(avatar_pred > 0.0);
    }
}
