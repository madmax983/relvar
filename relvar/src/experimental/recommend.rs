//! Relational Recommender System.
//!
//! This module implements Collaborative Filtering using purely relational algebra
//! operations. It demonstrates how complex analytical tasks like recommendations
//! can be computed directly within a relational engine without extracting data.
//!
//! # Concept
//!
//! We represent user-item interactions (e.g., ratings or views) as a relation:
//! `(user_id, item_id, score)`.
//!
//! Using relational operators, we can:
//! 1. Find users who rated the same items (`Join`).
//! 2. Compute similarity between items or users based on co-occurrence or ratings (`Extend`, `Summarize`).
//! 3. Predict scores for unseen items for a target user.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A Collaborative Filtering Recommender.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::CollaborativeFilter;
/// // Note: This is a placeholder example
/// ```
pub struct CollaborativeFilter {
    /// The ratings relation. Schema: (user_id_attr, item_id_attr, score_attr)
    ratings: Relation,
    user_id_attr: String,
    item_id_attr: String,
    score_attr: String,
}

impl CollaborativeFilter {
    /// Creates a new CollaborativeFilter.
    ///
    /// # Arguments
    ///
    /// * `ratings` - The relation containing user-item ratings.
    /// * `user_id_attr` - The attribute name for the user identifier.
    /// * `item_id_attr` - The attribute name for the item identifier.
    /// * `score_attr` - The attribute name for the rating/score (must be numeric, ideally Float).
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::CollaborativeFilter;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(
        ratings: Relation,
        user_id_attr: &str,
        item_id_attr: &str,
        score_attr: &str,
    ) -> Self {
        Self {
            ratings,
            user_id_attr: user_id_attr.to_string(),
            item_id_attr: item_id_attr.to_string(),
            score_attr: score_attr.to_string(),
        }
    }

    /// Recommends items for a target user using a User-Based Collaborative Filtering approach.
    ///
    /// Yields a relation with heading `(item_id_attr, predicted_score)` containing
    /// items the user has not yet rated. (The caller must sort if ordering is desired).
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::CollaborativeFilter;
    /// // Note: This is a placeholder example
    /// ```
    pub fn recommend_user_based(
        &self,
        target_user_id: ScalarValue,
    ) -> Result<Relation, DatabaseError> {
        // Step 1: Find items the target user HAS rated
        let target_user_ratings = self
            .ratings
            .restrict(|t| t.get(&self.user_id_attr) == Some(&target_user_id));

        if target_user_ratings.is_empty() {
            let heading = TupleType::new()
                .with_attribute(
                    self.item_id_attr.clone(),
                    self.ratings
                        .relation_type()
                        .heading()
                        .get_attribute_type(&self.item_id_attr)
                        .unwrap()
                        .clone(),
                )
                .with_attribute("predicted_score".to_string(), ScalarType::Float);
            return Ok(Relation::new(RelationType::new(heading)));
        }

        // Project out target user ratings: (item_id, target_score)
        let target_items = target_user_ratings
            .project(&[self.item_id_attr.as_str(), self.score_attr.as_str()])
            .rename(&[(self.score_attr.as_str(), "target_score")]);

        // Step 2: Find all other users who rated those same items and compute similarity.
        let user_similarity = self.compute_user_similarities(&target_user_id, &target_items)?;

        // Step 3: Find items rated by these similar users, but NOT by the target user.
        let unseen_ratings = self.find_unseen_ratings(&target_user_id, &target_items)?;

        // Step 4: Predict score for unseen items based on similar users' ratings.
        self.predict_unseen_items(user_similarity, unseen_ratings)
    }

    fn compute_similarity_products(
        &self,
        target_user_id: &ScalarValue,
        target_items: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let similar_users_items = target_items.join(&self.ratings)?;
        let similar_users_items =
            similar_users_items.restrict(|t| t.get(&self.user_id_attr) != Some(target_user_id));

        similar_users_items
            .extend("products", ScalarType::Float, |t| {
                let s1 = t.get_typed::<f64>("target_score").unwrap_or(0.0);
                let s2 = t.get_typed::<f64>(&self.score_attr).unwrap_or(0.0);
                ScalarValue::Float(s1 * s2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sq_target", ScalarType::Float, |t| {
                let s = t.get_typed::<f64>("target_score").unwrap_or(0.0);
                ScalarValue::Float(s * s)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sq_other", ScalarType::Float, |t| {
                let s = t.get_typed::<f64>(&self.score_attr).unwrap_or(0.0);
                ScalarValue::Float(s * s)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn summarize_and_calculate_similarity(
        &self,
        products: Relation,
    ) -> Result<Relation, DatabaseError> {
        let user_similarity_sums = products
            .summarize(
                &[self.user_id_attr.as_str()],
                &[
                    Aggregation::sum_float("sum_products", "products"),
                    Aggregation::sum_float("sum_sq_target", "sq_target"),
                    Aggregation::sum_float("sum_sq_other", "sq_other"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        user_similarity_sums
            .extend("similarity", ScalarType::Float, |t| {
                let sum_prod = t.get_typed::<f64>("sum_products").unwrap_or(0.0);
                let sum_sq_t = t.get_typed::<f64>("sum_sq_target").unwrap_or(0.0);
                let sum_sq_o = t.get_typed::<f64>("sum_sq_other").unwrap_or(0.0);

                let denom = (sum_sq_t * sum_sq_o).sqrt();
                let sim = if denom == 0.0 { 0.0 } else { sum_prod / denom };
                ScalarValue::Float(sim)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
            .map(|r| r.project(&[self.user_id_attr.as_str(), "similarity"]))
    }

    fn compute_user_similarities(
        &self,
        target_user_id: &ScalarValue,
        target_items: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let products = self.compute_similarity_products(target_user_id, target_items)?;
        self.summarize_and_calculate_similarity(products)
    }

    fn find_unseen_ratings(
        &self,
        target_user_id: &ScalarValue,
        target_items: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // target_items_ids: (item_id)
        let target_item_ids = target_items.project(&[self.item_id_attr.as_str()]);

        // all_other_ratings: (user_id, item_id, score)
        // Restrict out target user's own ratings.
        let other_users_ratings = self
            .ratings
            .restrict(|t| t.get(&self.user_id_attr) != Some(target_user_id));

        // Filter out items the target user already rated.
        // To do this, we join with all items NOT in target_item_ids.
        // Relational algebra difference:
        // unseen_item_ids = all_item_ids MINUS target_item_ids
        let all_item_ids = self.ratings.project(&[self.item_id_attr.as_str()]);
        let unseen_item_ids = all_item_ids
            .difference(&target_item_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Join to keep only ratings for unseen items
        // unseen_ratings: (user_id, item_id, score)
        other_users_ratings.join(&unseen_item_ids)
    }

    fn compute_prediction_components(
        &self,
        user_similarity: Relation,
        unseen_ratings: Relation,
    ) -> Result<Relation, DatabaseError> {
        let similarity_ratings = unseen_ratings.join(&user_similarity)?;

        similarity_ratings
            .extend("sim_score", ScalarType::Float, |t| {
                let sim = t.get_typed::<f64>("similarity").unwrap_or(0.0);
                let s = t.get_typed::<f64>(&self.score_attr).unwrap_or(0.0);
                ScalarValue::Float(sim * s)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("abs_sim", ScalarType::Float, |t| {
                let sim = t.get_typed::<f64>("similarity").unwrap_or(0.0);
                ScalarValue::Float(sim.abs())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn summarize_and_calculate_predictions(
        &self,
        extended_ratings: Relation,
    ) -> Result<Relation, DatabaseError> {
        let item_predictions = extended_ratings
            .summarize(
                &[self.item_id_attr.as_str()],
                &[
                    Aggregation::sum_float("sum_sim_score", "sim_score"),
                    Aggregation::sum_float("sum_abs_sim", "abs_sim"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        item_predictions
            .extend("predicted_score", ScalarType::Float, |t| {
                let num = t.get_typed::<f64>("sum_sim_score").unwrap_or(0.0);
                let den = t.get_typed::<f64>("sum_abs_sim").unwrap_or(0.0);

                let pred = if den == 0.0 { 0.0 } else { num / den };
                ScalarValue::Float(pred)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
            .map(|r| r.project(&[self.item_id_attr.as_str(), "predicted_score"]))
    }

    fn predict_unseen_items(
        &self,
        user_similarity: Relation,
        unseen_ratings: Relation,
    ) -> Result<Relation, DatabaseError> {
        let extended_ratings =
            self.compute_prediction_components(user_similarity, unseen_ratings)?;
        self.summarize_and_calculate_predictions(extended_ratings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_recommend_collaborative_filtering() {
        // Schema: (user_id: Int, item_id: Int, rating: Float)
        let heading = TupleType::new()
            .with_attribute("user_id", ScalarType::Int)
            .with_attribute("item_id", ScalarType::Int)
            .with_attribute("rating", ScalarType::Float);

        let mut ratings = Relation::new(RelationType::new(heading));

        // Dataset
        // User 1 likes items 1 and 2
        ratings
            .insert(tuple! { user_id: 1i64, item_id: 1i64, rating: 5.0 })
            .unwrap();
        ratings
            .insert(tuple! { user_id: 1i64, item_id: 2i64, rating: 4.0 })
            .unwrap();

        // User 2 likes items 1, 2, and 3
        ratings
            .insert(tuple! { user_id: 2i64, item_id: 1i64, rating: 4.5 })
            .unwrap();
        ratings
            .insert(tuple! { user_id: 2i64, item_id: 2i64, rating: 4.0 })
            .unwrap();
        ratings
            .insert(tuple! { user_id: 2i64, item_id: 3i64, rating: 5.0 })
            .unwrap();

        // User 3 likes items 2 and 4
        ratings
            .insert(tuple! { user_id: 3i64, item_id: 2i64, rating: 3.5 })
            .unwrap();
        ratings
            .insert(tuple! { user_id: 3i64, item_id: 4i64, rating: 4.0 })
            .unwrap();

        let recommender = CollaborativeFilter::new(ratings, "user_id", "item_id", "rating");

        // Recommend for User 1
        // User 1 has high similarity with User 2 (co-rated 1 and 2).
        // User 2 rated item 3 high.
        // User 1 has moderate similarity with User 3 (co-rated 2).
        // User 3 rated item 4 high.
        // Expect recommendations to include item 3 and 4.

        let recs = recommender
            .recommend_user_based(ScalarValue::Int(1))
            .unwrap();

        assert_eq!(recs.cardinality(), 2);

        let mut recs_vec: Vec<_> = recs.tuples().collect();
        // Sort by item_id just for deterministic assertion
        recs_vec.sort_by_key(|t| t.get_typed::<i64>("item_id").unwrap());

        let t3 = recs_vec[0];
        assert_eq!(t3.get_typed::<i64>("item_id").unwrap(), 3);
        assert!(t3.get_typed::<f64>("predicted_score").unwrap() > 4.0);

        let t4 = recs_vec[1];
        assert_eq!(t4.get_typed::<i64>("item_id").unwrap(), 4);
        assert!(t4.get_typed::<f64>("predicted_score").unwrap() > 3.0);
    }
}
