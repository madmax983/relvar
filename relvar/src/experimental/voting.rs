//! Relational Voting System (Condorcet Method)
//!
//! This module demonstrates how to implement a Ranked Choice Voting system, specifically
//! the Condorcet method, using pure relational algebra.
//!
//! # Concept
//!
//! We represent voter preferences as a relation of `(voter: Int, candidate: String, rank: Int)`.
//! The Condorcet winner is the candidate who would win a 1-on-1 election against every other candidate.
//!
//! The algorithm:
//! 1. Generate all pairs of candidates `(c1, c2)`.
//! 2. Join with the `preferences` relation for both `c1` and `c2` to compare their ranks.
//! 3. Use `Extend` and `Summarize` to count how many voters prefer `c1` over `c2` (giving `wins`)
//!    and `c2` over `c1` (giving `losses`).
//! 4. Identify `defeats` where `wins < losses`.
//! 5. The Condorcet winner is the candidate who is never defeated.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// Evaluates a Condorcet election using pure relational algebra.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::voting::CondorcetElection;
///
/// let prefs_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("voter".to_string(), ScalarType::Int)
///         .with_attribute("candidate".to_string(), ScalarType::String)
///         .with_attribute("rank".to_string(), ScalarType::Int)
/// );
/// let mut prefs = Relation::new(prefs_type);
///
/// // Voter 1: A > B > C
/// prefs.insert(tuple! { voter: 1i64, candidate: "A", rank: 1i64 }).unwrap();
/// prefs.insert(tuple! { voter: 1i64, candidate: "B", rank: 2i64 }).unwrap();
/// prefs.insert(tuple! { voter: 1i64, candidate: "C", rank: 3i64 }).unwrap();
///
/// // Voter 2: B > C > A
/// prefs.insert(tuple! { voter: 2i64, candidate: "B", rank: 1i64 }).unwrap();
/// prefs.insert(tuple! { voter: 2i64, candidate: "C", rank: 2i64 }).unwrap();
/// prefs.insert(tuple! { voter: 2i64, candidate: "A", rank: 3i64 }).unwrap();
///
/// // Voter 3: B > A > C
/// prefs.insert(tuple! { voter: 3i64, candidate: "B", rank: 1i64 }).unwrap();
/// prefs.insert(tuple! { voter: 3i64, candidate: "A", rank: 2i64 }).unwrap();
/// prefs.insert(tuple! { voter: 3i64, candidate: "C", rank: 3i64 }).unwrap();
///
/// // Evaluate Condorcet
/// let election = CondorcetElection::new(prefs);
/// let winners = election.evaluate().unwrap();
///
/// // Candidate B beats A (2-1) and beats C (3-0), so B is the Condorcet winner.
/// assert_eq!(winners.cardinality(), 1);
/// let winner_tuple = winners.tuples().next().unwrap();
/// assert_eq!(winner_tuple.get_typed::<String>("candidate").unwrap(), "B");
/// ```
pub struct CondorcetElection {
    /// The preferences relation. Schema: (voter: Int, candidate: String, rank: Int)
    pub preferences: Relation,
}

impl CondorcetElection {
    /// Creates a new Condorcet Election from a preferences relation.
    pub fn new(preferences: Relation) -> Self {
        Self { preferences }
    }

    /// Evaluates the election and returns a relation of Condorcet winners.
    /// In a strict Condorcet setup with no cycles, this contains exactly one candidate.
    ///
    /// Schema of returned relation: `(candidate: String)`
    pub fn evaluate(&self) -> Result<Relation, DatabaseError> {
        let prefs = &self.preferences;

        // 1. Extract all unique candidates
        let candidates = prefs.clone().project_into(&["candidate"]);

        // 2. Generate all pairs of candidates (c1, c2) where c1 != c2
        let c1 = candidates.clone().rename_into(&[("candidate", "c1")]);
        let c2 = candidates.clone().rename_into(&[("candidate", "c2")]);

        let pairs = c1.join(&c2)?.restrict_into(|t| {
            let name1 = t.get_typed::<String>("c1").unwrap();
            let name2 = t.get_typed::<String>("c2").unwrap();
            name1 != name2
        });

        // 3. Prepare voter preferences for c1 and c2
        let p1 = prefs
            .clone()
            .rename_into(&[("candidate", "c1"), ("rank", "rank1")]);
        let p2 = prefs
            .clone()
            .rename_into(&[("candidate", "c2"), ("rank", "rank2")]);

        // 4. Matchup voters' ranks for each pair of candidates
        // This joins on c1, c2 (from pairs) and voter (from p1 and p2)
        let matchups = pairs.join(&p1)?.join(&p2)?;

        // 5. Score the matchup: 1 if c1 is preferred (lower rank), 0 otherwise
        let matchups_scored = matchups
            .extend_into("score", ScalarType::Int, |t| {
                let r1 = t.get_typed::<i64>("rank1").unwrap();
                let r2 = t.get_typed::<i64>("rank2").unwrap();
                if r1 < r2 {
                    ScalarValue::Int(1)
                } else {
                    ScalarValue::Int(0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Summarize to sum the scores across all voters for each pair
        let pairwise_wins = matchups_scored
            .summarize(&["c1", "c2"], &[Aggregation::sum("wins", "score")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 7. Compare wins vs losses for each pair
        let w1 = pairwise_wins.clone();
        let w2 = pairwise_wins.rename_into(&[("c1", "c2"), ("c2", "c1"), ("wins", "losses")]);

        let comparisons = w1.join(&w2)?;

        // 8. Find matchups where c1 was defeated by c2
        let defeats = comparisons.restrict_into(|t| {
            let wins = t.get_typed::<i64>("wins").unwrap();
            let losses = t.get_typed::<i64>("losses").unwrap();
            wins < losses
        });

        // 9. A candidate is defeated if they lost any matchup
        let defeated_candidates = defeats
            .project_into(&["c1"])
            .rename_into(&[("c1", "candidate")]);

        // 10. The Condorcet winner is the candidate who was never defeated
        let winners = candidates
            .difference(&defeated_candidates)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(winners)
    }
}
