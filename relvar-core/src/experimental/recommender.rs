//! Relational Recommender System (Collaborative Filtering)
//!
//! This module implements a basic user-based collaborative filtering recommender
//! system using purely relational algebra.

use crate::algebra::Aggregation;
use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue, Tuple};

/// Computes the cosine similarity between all pairs of users.
///
/// `ratings` must have attributes: `user` (Int), `item` (Int), `rating` (Float).
pub fn compute_similarities(ratings: &Relation) -> Result<Relation, DatabaseError> {
    // 1. Rename to distinguish the two users
    let ratings1 = ratings.rename(&[("user", "u1"), ("rating", "r1")]);
    let ratings2 = ratings.rename(&[("user", "u2"), ("rating", "r2")]);

    // 2. Self-join on `item` to find items rated by both users
    let common_ratings = ratings1
        .join(&ratings2)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Filter out symmetric pairs and self-pairs to save computation (u1 < u2)
    let different_users = common_ratings.restrict(|t| {
        let u1 = t.get_typed::<i64>("u1").unwrap_or(0);
        let u2 = t.get_typed::<i64>("u2").unwrap_or(0);
        u1 != u2
    });

    // 3. Extend to compute components of cosine similarity
    let ext1 = different_users
        .extend("r1_r2", ScalarType::Float, |t: &Tuple| {
            let r1 = t.get_typed::<f64>("r1").unwrap_or(0.0);
            let r2 = t.get_typed::<f64>("r2").unwrap_or(0.0);
            ScalarValue::Float(r1 * r2)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let ext2 = ext1
        .extend("r1_sq", ScalarType::Float, |t: &Tuple| {
            let r1 = t.get_typed::<f64>("r1").unwrap_or(0.0);
            ScalarValue::Float(r1 * r1)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let extended = ext2
        .extend("r2_sq", ScalarType::Float, |t: &Tuple| {
            let r2 = t.get_typed::<f64>("r2").unwrap_or(0.0);
            ScalarValue::Float(r2 * r2)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 4. Summarize by u1 and u2
    let sums = extended
        .summarize(
            &["u1", "u2"],
            &[
                Aggregation::sum_float("sum_r1_r2", "r1_r2"),
                Aggregation::sum_float("sum_r1_sq", "r1_sq"),
                Aggregation::sum_float("sum_r2_sq", "r2_sq"),
            ],
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. Compute final similarity
    let similarities = sums
        .extend("similarity", ScalarType::Float, |t: &Tuple| {
            let dot_product = t.get_typed::<f64>("sum_r1_r2").unwrap_or(0.0);
            let norm1_sq = t.get_typed::<f64>("sum_r1_sq").unwrap_or(1.0);
            let norm2_sq = t.get_typed::<f64>("sum_r2_sq").unwrap_or(1.0);

            let denom = (norm1_sq * norm2_sq).sqrt();
            let sim = if denom == 0.0 {
                0.0
            } else {
                dot_product / denom
            };
            ScalarValue::Float(sim)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Project only u1, u2, similarity
    Ok(similarities.project(&["u1", "u2", "similarity"]))
}

/// Predicts ratings for unseen items based on user similarities.
///
/// `ratings` must have attributes: `user` (Int), `item` (Int), `rating` (Float).
/// `similarities` must have attributes: `u1` (Int), `u2` (Int), `similarity` (Float).
pub fn predict_ratings(
    ratings: &Relation,
    similarities: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Join similarities with ratings of u2
    let ratings_u2 = ratings.rename(&[("user", "u2")]);

    let joined = similarities
        .join(&ratings_u2)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Compute similarity * rating
    let ext1 = joined
        .extend("sim_rating", ScalarType::Float, |t: &Tuple| {
            let sim = t.get_typed::<f64>("similarity").unwrap_or(0.0);
            let rating = t.get_typed::<f64>("rating").unwrap_or(0.0);
            ScalarValue::Float(sim * rating)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let extended = ext1
        .extend("abs_sim", ScalarType::Float, |t: &Tuple| {
            let sim = t.get_typed::<f64>("similarity").unwrap_or(0.0);
            ScalarValue::Float(sim.abs())
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Summarize by u1 and item
    let sums = extended
        .summarize(
            &["u1", "item"],
            &[
                Aggregation::sum_float("sum_sim_rating", "sim_rating"),
                Aggregation::sum_float("sum_abs_sim", "abs_sim"),
            ],
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 4. Compute predicted rating
    let predictions = sums
        .extend("predicted_rating", ScalarType::Float, |t: &Tuple| {
            let sum_sim_rating = t.get_typed::<f64>("sum_sim_rating").unwrap_or(0.0);
            let sum_abs_sim = t.get_typed::<f64>("sum_abs_sim").unwrap_or(1.0);

            let pred = if sum_abs_sim == 0.0 {
                0.0
            } else {
                sum_sim_rating / sum_abs_sim
            };
            ScalarValue::Float(pred)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. Exclude items the user has already rated
    let user_rated = ratings.rename(&[("user", "u1")]).project(&["u1", "item"]);
    let final_preds = predictions.project(&["u1", "item", "predicted_rating"]);

    let to_remove_full = final_preds
        .join(&user_rated)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let final_preds_unrated = final_preds
        .difference(&to_remove_full)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(final_preds_unrated.rename(&[("u1", "user")]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    #[test]
    fn test_recommender() {
        let heading = TupleType::new()
            .with_attribute("user", ScalarType::Int)
            .with_attribute("item", ScalarType::Int)
            .with_attribute("rating", ScalarType::Float);
        let mut ratings = Relation::new(RelationType::new(heading));

        // User 1 likes items 1 and 2
        ratings
            .insert(tuple! {user: 1i64, item: 1i64, rating: 5.0})
            .unwrap();
        ratings
            .insert(tuple! {user: 1i64, item: 2i64, rating: 4.0})
            .unwrap();

        // User 2 likes items 1, 2, and 3
        ratings
            .insert(tuple! {user: 2i64, item: 1i64, rating: 4.0})
            .unwrap();
        ratings
            .insert(tuple! {user: 2i64, item: 2i64, rating: 5.0})
            .unwrap();
        ratings
            .insert(tuple! {user: 2i64, item: 3i64, rating: 4.5})
            .unwrap();

        // User 3 likes items 2 and 3
        ratings
            .insert(tuple! {user: 3i64, item: 2i64, rating: 1.0})
            .unwrap();

        let similarities = compute_similarities(&ratings).unwrap();
        assert!(similarities.cardinality() > 0);

        let predictions = predict_ratings(&ratings, &similarities).unwrap();

        let u1_i3 = predictions.restrict(|t| {
            t.get_typed::<i64>("user") == Some(1) && t.get_typed::<i64>("item") == Some(3)
        });

        assert_eq!(u1_i3.cardinality(), 1);
        let pred_tuple = u1_i3.tuples().next().unwrap();
        let pred_rating = pred_tuple.get_typed::<f64>("predicted_rating").unwrap();
        assert!(pred_rating > 3.0);
    }
}
