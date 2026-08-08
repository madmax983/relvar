//! Relational Wordle Game Logic
//!
//! This module models Wordle game logic using relational algebra.
//! Words and letters are stored in relations.
//!
//! # Concept
//!
//! - **Target Word**: Relation `(position: Int, letter: String)`
//! - **Guess Word**: Relation `(position: Int, letter: String)`
//!
//! The goal is to compute the result of a guess:
//! - Exact Match (Green): Same position and same letter.
//! - Partial Match (Yellow): Same letter, different position (with frequency constraints).
//! - Miss (Gray): Letter not in target word, or frequency exhausted.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Wordle logic evaluator.
///
/// # Concept
/// Words are converted into relations with schema `(position: Int, letter: String)`.
/// The evaluation proceeds iteratively using purely set-based algebra.
///
/// # Examples
/// ```
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::{Relation, ScalarValue, Tuple};
/// use relvar::experimental::wordle::WordleEngine;
///
/// let engine = WordleEngine::new();
/// let target = engine.word_to_relation("apple").unwrap();
/// let guess = engine.word_to_relation("paper").unwrap();
/// let result = engine.evaluate_guess(&target, &guess).unwrap();
/// ```
pub struct WordleEngine {
    /// Schema: (position: Int, letter: String)
    target_type: RelationType,
}

impl WordleEngine {
    /// Create a new Wordle engine
    pub fn new() -> Self {
        let target_type = RelationType::new(
            TupleType::new()
                .with_attribute("position", ScalarType::Int)
                .with_attribute("letter", ScalarType::String),
        );
        Self { target_type }
    }

    /// Helper to convert a string to a relation of (position, letter)
    pub fn word_to_relation(&self, word: &str) -> Result<Relation, DatabaseError> {
        let mut rel = Relation::new(self.target_type.clone());
        for (i, c) in word.chars().enumerate() {
            let t = tuple! {
                position: i as i64,
                letter: c.to_string()
            };
            rel.insert(t)?;
        }
        Ok(rel)
    }

    /// Evaluate a guess against a target word.
    /// Returns a relation of (position: Int, letter: String, status: String)
    pub fn evaluate_guess(
        &self,
        target: &Relation,
        guess: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let exact_matches_join = guess
            .clone()
            .join(target)
            .map_err(|_| DatabaseError::AlgebraError("join".to_string()))?;
        let exact_matches = exact_matches_join
            .extend("status", ScalarType::String, |_| {
                ScalarValue::String("Exact".to_string())
            })
            .map_err(|_| DatabaseError::AlgebraError("extend".to_string()))?;

        let exact_pos_letter = exact_matches.clone().project(&["position", "letter"]);
        let target_remaining = target
            .clone()
            .difference(&exact_pos_letter)
            .map_err(|_| DatabaseError::AlgebraError("difference".to_string()))?;

        let target_avail_counts = target_remaining
            .summarize(&["letter"], &[Aggregation::count("target_count")])
            .map_err(|_| DatabaseError::AlgebraError("summarize".to_string()))?;

        let guess_remaining = guess
            .clone()
            .difference(&exact_pos_letter)
            .map_err(|_| DatabaseError::AlgebraError("difference".to_string()))?;

        let g1 = guess_remaining
            .clone()
            .rename(&[("position", "pos1"), ("letter", "let1")]);
        let g2 = guess_remaining
            .clone()
            .rename(&[("position", "pos2"), ("letter", "let2")]);

        let strictly_earlier = g1.theta_join(&g2, |t1: &Tuple, t2: &Tuple| {
            let pos1 = t1.get_typed::<i64>("pos1").unwrap();
            let pos2 = t2.get_typed::<i64>("pos2").unwrap();
            let let1 = t1.get_typed::<String>("let1").unwrap();
            let let2 = t2.get_typed::<String>("let2").unwrap();
            let1 == let2 && pos2 < pos1
        });

        let earlier_counts = strictly_earlier
            .summarize(&["pos1", "let1"], &[Aggregation::count("earlier")])
            .map_err(|_| DatabaseError::AlgebraError("summarize".to_string()))?;

        let has_earlier = earlier_counts.clone().project(&["pos1", "let1"]);
        let no_earlier = g1
            .difference(&has_earlier)
            .map_err(|_| DatabaseError::AlgebraError("difference".to_string()))?;

        let no_earlier_counts = no_earlier
            .extend("earlier", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|_| DatabaseError::AlgebraError("extend".to_string()))?;

        let earlier_union = earlier_counts
            .union(&no_earlier_counts)
            .map_err(|_| DatabaseError::AlgebraError("union".to_string()))?;

        let all_ranks_extended = earlier_union
            .extend("rank", ScalarType::Int, |t: &Tuple| {
                let e = t.get_typed::<i64>("earlier").unwrap();
                ScalarValue::Int(e + 1)
            })
            .map_err(|_| DatabaseError::AlgebraError("extend".to_string()))?;

        let all_ranks_projected = all_ranks_extended.project(&["pos1", "let1", "rank"]);
        let all_ranks = all_ranks_projected.rename(&[("pos1", "position"), ("let1", "letter")]);

        let joined = all_ranks
            .clone()
            .join(&target_avail_counts)
            .map_err(|_| DatabaseError::AlgebraError("join".to_string()))?;

        let partial_matches_restricted = joined.clone().restrict_into(|t: &Tuple| {
            let rank = t.get_typed::<i64>("rank").unwrap();
            let count = t.get_typed::<i64>("target_count").unwrap();
            rank <= count
        });

        let partial_matches = partial_matches_restricted
            .project(&["position", "letter"])
            .extend("status", ScalarType::String, |_| {
                ScalarValue::String("Partial".to_string())
            })
            .map_err(|_| DatabaseError::AlgebraError("extend".to_string()))?;

        let miss_from_rank_restricted = joined.clone().restrict_into(|t: &Tuple| {
            let rank = t.get_typed::<i64>("rank").unwrap();
            let count = t.get_typed::<i64>("target_count").unwrap();
            rank > count
        });

        let miss_from_rank = miss_from_rank_restricted.project(&["position", "letter"]);

        let letters_in_target = target_avail_counts.project(&["letter"]);
        let all_ranks_pos_let = all_ranks.clone().project(&["position", "letter"]);

        let all_ranks_in_target_joined = all_ranks
            .join(&letters_in_target)
            .map_err(|_| DatabaseError::AlgebraError("join".to_string()))?;
        let all_ranks_in_target = all_ranks_in_target_joined.project(&["position", "letter"]);

        let miss_not_in_target = all_ranks_pos_let
            .difference(&all_ranks_in_target)
            .map_err(|_| DatabaseError::AlgebraError("difference".to_string()))?;

        let miss_union = miss_from_rank
            .union(&miss_not_in_target)
            .map_err(|_| DatabaseError::AlgebraError("union".to_string()))?;

        let all_misses = miss_union
            .extend("status", ScalarType::String, |_| {
                ScalarValue::String("Miss".to_string())
            })
            .map_err(|_| DatabaseError::AlgebraError("extend".to_string()))?;

        let final_result1 = exact_matches
            .union(&partial_matches)
            .map_err(|_| DatabaseError::AlgebraError("union".to_string()))?;

        let final_result = final_result1
            .union(&all_misses)
            .map_err(|_| DatabaseError::AlgebraError("union".to_string()))?;

        Ok(final_result)
    }
}

impl Default for WordleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wordle_exact() -> Result<(), DatabaseError> {
        let engine = WordleEngine::new();
        let target = engine.word_to_relation("apple")?;
        let guess = engine.word_to_relation("apple")?;

        let result = engine.evaluate_guess(&target, &guess)?;

        assert_eq!(result.cardinality(), 5);
        let exacts =
            result.restrict_into(|t: &Tuple| t.get_typed::<String>("status").unwrap() == "Exact");
        assert_eq!(exacts.cardinality(), 5);

        Ok(())
    }

    #[test]
    fn test_wordle_complex() -> Result<(), DatabaseError> {
        let engine = WordleEngine::new();
        let target = engine.word_to_relation("apple")?;
        let guess = engine.word_to_relation("paper")?;

        let result = engine.evaluate_guess(&target, &guess)?;

        let partials = result
            .clone()
            .restrict_into(|t: &Tuple| t.get_typed::<String>("status").unwrap() == "Partial");
        assert_eq!(partials.cardinality(), 3);

        let exacts = result
            .clone()
            .restrict_into(|t: &Tuple| t.get_typed::<String>("status").unwrap() == "Exact");
        assert_eq!(exacts.cardinality(), 1);

        let misses = result
            .clone()
            .restrict_into(|t: &Tuple| t.get_typed::<String>("status").unwrap() == "Miss");
        assert_eq!(misses.cardinality(), 1);

        Ok(())
    }

    #[test]
    fn test_wordle_duplicate_guess_letters() -> Result<(), DatabaseError> {
        let engine = WordleEngine::new();
        let target = engine.word_to_relation("apple")?;
        let guess = engine.word_to_relation("puppy")?;

        let result = engine.evaluate_guess(&target, &guess)?;

        let exact_p = result.clone().restrict_into(|t: &Tuple| {
            t.get_typed::<String>("status").unwrap() == "Exact"
                && t.get_typed::<String>("letter").unwrap() == "p"
        });
        assert_eq!(exact_p.cardinality(), 1);

        let partial_p = result.clone().restrict_into(|t: &Tuple| {
            t.get_typed::<String>("status").unwrap() == "Partial"
                && t.get_typed::<String>("letter").unwrap() == "p"
        });
        assert_eq!(partial_p.cardinality(), 1);

        let miss_p = result.clone().restrict_into(|t: &Tuple| {
            t.get_typed::<String>("status").unwrap() == "Miss"
                && t.get_typed::<String>("letter").unwrap() == "p"
        });
        assert_eq!(miss_p.cardinality(), 1);

        Ok(())
    }
}
