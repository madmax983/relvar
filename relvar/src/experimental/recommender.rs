//! Relational Recommender System (Collaborative Filtering).
//!
//! This module demonstrates how a collaborative filtering recommendation engine can
//! be implemented using purely relational algebra. It computes user-user similarity
//! using the dot product of their item ratings, all without explicit array or vector structures.
//!
//! # Concept
//!
//! - **Ratings**: A relation storing user preferences: `(user, item, rating)`.
//! - **Similarity**: Computed by joining the ratings relation with itself on `item`,
//!   calculating the product of ratings for co-rated items, and summarizing the
//!   sum of these products per user pair (dot product).
//! - **Recommendations**: Extrapolated by joining a target user's similarities with
//!   other users' ratings for items the target user hasn't rated yet.
//!
//! # Usage
//!
//! ```rust
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::{RelationType, ScalarType, TupleType};
//! use relvar::{ScalarValue, Relation};
//! use relvar::experimental::recommender::CollaborativeFilter;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut db = Database::new(InMemoryEngine::new());
//!
//! // 1. Define Ratings Relation
//! let heading = TupleType::new()
//!     .with_attribute("user", ScalarType::String)
//!     .with_attribute("item", ScalarType::String)
//!     .with_attribute("rating", ScalarType::Float);
//! db.create_relvar("RATINGS", RelationType::new(heading))?;
//!
//! // 2. Insert Ratings Data
//! db.insert("RATINGS", tuple! { user: "Alice", item: "Matrix", rating: 5.0 })?;
//! db.insert("RATINGS", tuple! { user: "Alice", item: "Inception", rating: 4.0 })?;
//! db.insert("RATINGS", tuple! { user: "Bob", item: "Matrix", rating: 4.0 })?;
//! db.insert("RATINGS", tuple! { user: "Bob", item: "Dune", rating: 5.0 })?;
//! db.insert("RATINGS", tuple! { user: "Charlie", item: "Inception", rating: 5.0 })?;
//!
//! // 3. Compute Similarity
//! let ratings = db.query("RATINGS")?;
//! let similarities = CollaborativeFilter::user_similarity(&ratings, "user", "rating")?;
//!
//! // 4. Generate Recommendations
//! let recs = CollaborativeFilter::recommend(&ratings, &similarities, "Alice", "user", "item", "rating")?;
//!
//! // Alice should be recommended 'Dune' based on her similarity to Bob
//! assert_eq!(recs.cardinality(), 1);
//! # Ok(())
//! # }
//! ```

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Collaborative Filtering Recommender Engine.
pub struct CollaborativeFilter;

impl CollaborativeFilter {
    /// Computes user-user similarity using the dot product of their ratings.
    ///
    /// The input `ratings` relation must have the specified `user_attr` and
    /// `rating_attr` columns.
    pub fn user_similarity(
        ratings: &Relation,
        user_attr: &str,
        rating_attr: &str,
    ) -> Result<Relation, DatabaseError> {
        // R1: Rename user and rating to prepare for self-join
        let r1 = ratings
            .rename(&[(user_attr, "user1")])
            .rename(&[(rating_attr, "rating1")]);

        // R2: Rename user and rating to prepare for self-join
        let r2 = ratings
            .rename(&[(user_attr, "user2")])
            .rename(&[(rating_attr, "rating2")]);

        // Join on item_attr to find items rated by both users
        let joined = r1
            .join(&r2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Filter out self-comparisons (user1 == user2)
        // Note: Using a clone of the joined relation and restricting it
        let pairs = joined.restrict(|t| {
            if let (Some(ScalarValue::String(u1)), Some(ScalarValue::String(u2))) =
                (t.get("user1"), t.get("user2"))
            {
                u1 != u2
            } else {
                false
            }
        });

        // Compute the product of ratings: rating1 * rating2
        let with_product = pairs
            .extend("rating_product", ScalarType::Float, |t| {
                let r1 = match t.get("rating1").unwrap() {
                    ScalarValue::Int(i) => *i as f64,
                    ScalarValue::Float(f) => *f,
                    _ => 0.0,
                };
                let r2 = match t.get("rating2").unwrap() {
                    ScalarValue::Int(i) => *i as f64,
                    ScalarValue::Float(f) => *f,
                    _ => 0.0,
                };
                ScalarValue::Float(r1 * r2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize by user1 and user2 to get the dot product (sum of rating products)
        let similarity = with_product
            .summarize(
                &["user1", "user2"],
                &[Aggregation::sum_float("similarity_score", "rating_product")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(similarity)
    }

    /// Generates item recommendations for a specific user based on user similarities.
    pub fn recommend(
        ratings: &Relation,
        similarities: &Relation,
        target_user: &str,
        user_attr: &str,
        item_attr: &str,
        rating_attr: &str,
    ) -> Result<Relation, DatabaseError> {
        let target_user_str = target_user.to_string();

        // 1. Get similar users for the target user
        let user_sims = similarities.restrict(move |t| {
            if let Some(ScalarValue::String(u1)) = t.get("user1") {
                u1 == &target_user_str
            } else {
                false
            }
        });

        // 2. Get ratings from similar users
        let sim_users_ratings = ratings
            .rename(&[(user_attr, "user2")])
            .rename(&[(rating_attr, "rating2")]);

        // Join similarities with the other users' ratings
        let joined = user_sims
            .join(&sim_users_ratings)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Weight their ratings by similarity score
        let weighted_ratings = joined
            .extend("weighted_rating", ScalarType::Float, |t| {
                let sim = match t.get("similarity_score").unwrap() {
                    ScalarValue::Float(f) => *f,
                    ScalarValue::Int(i) => *i as f64,
                    _ => 0.0,
                };
                let rating = match t.get("rating2").unwrap() {
                    ScalarValue::Float(f) => *f,
                    ScalarValue::Int(i) => *i as f64,
                    _ => 0.0,
                };
                ScalarValue::Float(sim * rating)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize to get the predicted score for each item
        let predictions = weighted_ratings
            .summarize(
                &[item_attr],
                &[Aggregation::sum_float("predicted_score", "weighted_rating")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Filter out items the target user has already rated
        let target_user_str_2 = target_user.to_string();
        let target_user_ratings = ratings.restrict(move |t| {
            if let Some(ScalarValue::String(u)) = t.get(user_attr) {
                u == &target_user_str_2
            } else {
                false
            }
        });

        // Project just the items to do a difference (anti-join)
        let seen_items = target_user_ratings.project(&[item_attr]);

        // To remove seen items, we can use semidifference
        let recommendations = predictions.semidifference(&seen_items);

        Ok(recommendations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::values::Tuple;
    use relvar_core::{RelationType, TupleType};
    use std::collections::HashMap;

    fn make_rating(user: &str, item: &str, rating: f64) -> Tuple {
        let mut map = HashMap::new();
        map.insert("user".to_string(), ScalarValue::String(user.to_string()));
        map.insert("item".to_string(), ScalarValue::String(item.to_string()));
        map.insert("rating".to_string(), ScalarValue::Float(rating));

        let heading = TupleType::new()
            .with_attribute("user", ScalarType::String)
            .with_attribute("item", ScalarType::String)
            .with_attribute("rating", ScalarType::Float);

        Tuple::new(heading, map).unwrap()
    }

    #[test]
    fn test_collaborative_filtering() {
        let heading = TupleType::new()
            .with_attribute("user", ScalarType::String)
            .with_attribute("item", ScalarType::String)
            .with_attribute("rating", ScalarType::Float);
        let mut ratings = Relation::new(RelationType::new(heading));

        // Alice likes Action and Sci-Fi
        ratings.insert(make_rating("Alice", "Matrix", 5.0)).unwrap();
        ratings
            .insert(make_rating("Alice", "Inception", 4.0))
            .unwrap();

        // Bob likes Action and Sci-Fi too, and also likes Dune
        ratings.insert(make_rating("Bob", "Matrix", 4.0)).unwrap();
        ratings.insert(make_rating("Bob", "Dune", 5.0)).unwrap();

        // Charlie likes different things
        ratings
            .insert(make_rating("Charlie", "Inception", 5.0))
            .unwrap();
        ratings
            .insert(make_rating("Charlie", "Titanic", 4.0))
            .unwrap();

        // Compute similarities
        let similarities =
            CollaborativeFilter::user_similarity(&ratings, "user", "rating").unwrap();

        // Check that Alice and Bob have a high similarity score
        // Matrix: 5.0 * 4.0 = 20.0
        // (No other co-rated items for Alice and Bob)
        let alice_bob_sim = similarities.restrict(|t| {
            if let (Some(ScalarValue::String(u1)), Some(ScalarValue::String(u2))) =
                (t.get("user1"), t.get("user2"))
            {
                u1 == "Alice" && u2 == "Bob"
            } else {
                false
            }
        });

        assert_eq!(alice_bob_sim.cardinality(), 1);
        let sim_tuple = alice_bob_sim.tuples().next().unwrap();
        let score = match sim_tuple.get("similarity_score").unwrap() {
            ScalarValue::Float(f) => *f,
            _ => panic!("Expected float"),
        };
        assert_eq!(score, 20.0);

        // Generate recommendations for Alice
        let recs = CollaborativeFilter::recommend(
            &ratings,
            &similarities,
            "Alice",
            "user",
            "item",
            "rating",
        )
        .unwrap();

        // Alice should be recommended Dune and Titanic
        // Dune comes from Bob (Sim=20.0, Rating=5.0 -> Score=100.0)
        // Titanic comes from Charlie (Sim=20.0, Rating=4.0 -> Score=80.0)
        assert_eq!(recs.cardinality(), 2);
    }
}
