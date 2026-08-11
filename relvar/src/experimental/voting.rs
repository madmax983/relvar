//! Relational Voting System
//!
//! This module models election evaluation using pure relational algebra.
//! It implements methods like Borda Count and Condorcet to determine
//! election winners purely declaratively.
//!
//! # Concepts
//! - **Candidates**: The individuals running for election.
//! - **Ballots**: The ranked preferences of voters (1 is top choice).
//!
//! By using joins to pair candidates against each other, and aggregations to
//! count pairwise preferences, we can resolve complex voting systems in a database query.

use relvar_core::algebra::Aggregation;
use relvar_core::{DatabaseError, Relation, ScalarType, ScalarValue};

/// A relational engine for resolving complex election systems.
pub struct VotingEngine {
    /// Relation containing the candidates.
    pub candidates: Relation,
    /// Relation containing the ranked ballots.
    pub ballots: Relation,
}

impl VotingEngine {
    /// Creates a new VotingEngine.
    pub fn new(candidates: Relation, ballots: Relation) -> Self {
        Self {
            candidates,
            ballots,
        }
    }

    /// Evaluates the election using the Borda Count method.
    pub fn borda_scores(&self) -> Result<Relation, DatabaseError> {
        let n = self.candidates.cardinality() as i64;

        let with_points = self
            .ballots
            .extend("points", ScalarType::Int, move |t| {
                let rank = t.get_typed::<i64>("rank").unwrap();
                ScalarValue::Int(n - rank)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        with_points
            .summarize(
                &["candidate"],
                &[Aggregation::sum("total_points", "points")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    /// Evaluates the election using the Condorcet method.
    pub fn condorcet_winner(&self) -> Result<Relation, DatabaseError> {
        let b1 = self.ballots.rename(&[("candidate", "c1"), ("rank", "r1")]);
        let b2 = self.ballots.rename(&[("candidate", "c2"), ("rank", "r2")]);

        let pairs = b1.join(&b2)?;

        let distinct_pairs = pairs.restrict(|t| {
            let c1 = t.get_typed::<String>("c1").unwrap();
            let c2 = t.get_typed::<String>("c2").unwrap();
            c1 != c2
        });

        let scored = distinct_pairs
            .extend("c1_score", ScalarType::Int, |t| {
                let r1 = t.get_typed::<i64>("r1").unwrap();
                let r2 = t.get_typed::<i64>("r2").unwrap();
                if r1 < r2 {
                    ScalarValue::Int(1)
                } else {
                    ScalarValue::Int(0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("c2_score", ScalarType::Int, |t| {
                let r1 = t.get_typed::<i64>("r1").unwrap();
                let r2 = t.get_typed::<i64>("r2").unwrap();
                if r2 < r1 {
                    ScalarValue::Int(1)
                } else {
                    ScalarValue::Int(0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let matchup_totals = scored
            .summarize(
                &["c1", "c2"],
                &[
                    Aggregation::sum("c1_votes", "c1_score"),
                    Aggregation::sum("c2_votes", "c2_score"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let c1_wins = matchup_totals.restrict(|t| {
            let v1 = t.get_typed::<i64>("c1_votes").unwrap();
            let v2 = t.get_typed::<i64>("c2_votes").unwrap();
            v1 > v2
        });

        let c1_win_counts = c1_wins
            .summarize(&["c1"], &[Aggregation::count("matchups_won")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let required_wins = self.candidates.cardinality() as i64 - 1;
        let winner = c1_win_counts
            .restrict(move |t| t.get_typed::<i64>("matchups_won").unwrap() == required_wins);

        Ok(winner.project(&["c1"]).rename(&[("c1", "candidate")]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_voting_engine() {
        let candidate_type =
            RelationType::new(TupleType::new().with_attribute("candidate", ScalarType::String));
        let mut candidates = Relation::new(candidate_type);
        candidates.insert(tuple! { candidate: "Alice" }).unwrap();
        candidates.insert(tuple! { candidate: "Bob" }).unwrap();
        candidates.insert(tuple! { candidate: "Charlie" }).unwrap();

        let ballot_type = RelationType::new(
            TupleType::new()
                .with_attribute("voter_id", ScalarType::Int)
                .with_attribute("candidate", ScalarType::String)
                .with_attribute("rank", ScalarType::Int),
        );
        let mut ballots = Relation::new(ballot_type);

        // Voter 1: Alice > Bob > Charlie
        ballots
            .insert(tuple! { voter_id: 1i64, candidate: "Alice", rank: 1i64 })
            .unwrap();
        ballots
            .insert(tuple! { voter_id: 1i64, candidate: "Bob", rank: 2i64 })
            .unwrap();
        ballots
            .insert(tuple! { voter_id: 1i64, candidate: "Charlie", rank: 3i64 })
            .unwrap();

        // Voter 2: Alice > Charlie > Bob
        ballots
            .insert(tuple! { voter_id: 2i64, candidate: "Alice", rank: 1i64 })
            .unwrap();
        ballots
            .insert(tuple! { voter_id: 2i64, candidate: "Charlie", rank: 2i64 })
            .unwrap();
        ballots
            .insert(tuple! { voter_id: 2i64, candidate: "Bob", rank: 3i64 })
            .unwrap();

        // Voter 3: Bob > Alice > Charlie
        ballots
            .insert(tuple! { voter_id: 3i64, candidate: "Bob", rank: 1i64 })
            .unwrap();
        ballots
            .insert(tuple! { voter_id: 3i64, candidate: "Alice", rank: 2i64 })
            .unwrap();
        ballots
            .insert(tuple! { voter_id: 3i64, candidate: "Charlie", rank: 3i64 })
            .unwrap();

        let engine = VotingEngine::new(candidates, ballots);

        // Borda Count:
        // Alice: (3-1) + (3-1) + (3-2) = 2 + 2 + 1 = 5
        // Bob: (3-2) + (3-3) + (3-1) = 1 + 0 + 2 = 3
        // Charlie: (3-3) + (3-2) + (3-3) = 0 + 1 + 0 = 1
        let borda = engine.borda_scores().unwrap();
        assert_eq!(borda.cardinality(), 3);

        let mut alice_borda = 0;
        for t in borda.tuples() {
            if t.get_typed::<String>("candidate").unwrap() == "Alice" {
                alice_borda = t.get_typed::<i64>("total_points").unwrap();
            }
        }
        assert_eq!(alice_borda, 5);

        // Condorcet:
        // Alice vs Bob: Alice wins 2-1
        // Alice vs Charlie: Alice wins 3-0
        // Bob vs Charlie: Bob wins 2-1
        // Alice wins all matchups -> Alice is Condorcet winner
        let condorcet = engine.condorcet_winner().unwrap();
        assert_eq!(condorcet.cardinality(), 1);
        let winner = condorcet.tuples().next().unwrap();
        assert_eq!(winner.get_typed::<String>("candidate").unwrap(), "Alice");
    }
}
