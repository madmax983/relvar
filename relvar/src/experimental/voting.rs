//! Relational Ranked Choice Voting (Instant-Runoff Voting)
//!
//! Models a ranked-choice voting system using purely relational algebra.
//! Ballots are represented as relations of preferences. The winner is determined
//! iteratively by counting the top-ranked active preferences, eliminating the candidate
//! with the fewest votes, and transferring votes, all via standard relational
//! operations (Join, Summarize, Difference).

use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::{DatabaseError, Relation, RelationType, ScalarType, TupleType};

/// Represents a Relational Ranked Choice Voting engine.
pub struct RankedChoiceVoting {
    /// Relation of all ballot preferences: `(voter_id: Int, candidate_id: String, rank: Int)`
    pub ballots: Relation,
    /// Relation of eliminated candidates: `(candidate_id: String)`
    pub eliminated: Relation,
}

impl Default for RankedChoiceVoting {
    fn default() -> Self {
        Self::new()
    }
}

impl RankedChoiceVoting {
    /// Creates a new RankedChoiceVoting engine.
    pub fn new() -> Self {
        let ballots_type = RelationType::new(
            TupleType::new()
                .with_attribute("voter_id".to_string(), ScalarType::Int)
                .with_attribute("candidate_id".to_string(), ScalarType::String)
                .with_attribute("rank".to_string(), ScalarType::Int),
        );

        let eliminated_type = RelationType::new(
            TupleType::new().with_attribute("candidate_id".to_string(), ScalarType::String),
        );

        Self {
            ballots: Relation::new(ballots_type),
            eliminated: Relation::new(eliminated_type),
        }
    }

    /// Evaluates the election, returning the winning candidate_id, or None if it's a tie/empty.
    pub fn evaluate(&mut self) -> Result<Option<String>, DatabaseError> {
        loop {
            // 1. Filter out eliminated candidates from ballots
            let eliminated_ballots = self
                .ballots
                .join(&self.eliminated)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            let valid_ballots = self
                .ballots
                .difference(&eliminated_ballots)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if valid_ballots.cardinality() == 0 {
                return Ok(None);
            }

            // 2. Find the highest remaining preference (minimum rank) for each voter
            let current_ranks = valid_ballots
                .summarize(
                    &["voter_id"],
                    &[Aggregation::min("min_rank", "rank", ScalarType::Int)],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Rename min_rank to rank to join back with valid_ballots
            let current_ranks_renamed = current_ranks.rename(&[("min_rank", "rank")]);

            // 3. Get the active votes by joining valid_ballots with their top current rank
            let active_votes = valid_ballots
                .join(&current_ranks_renamed)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 4. Count the votes for each candidate
            let vote_counts = active_votes
                .summarize(&["candidate_id"], &[Aggregation::count("votes")])
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            let total_votes = active_votes.cardinality();
            let threshold = total_votes / 2;

            // 5. Check if we have a winner
            let mut max_votes = -1;
            let mut winner = None;
            let mut min_votes = i64::MAX;
            let mut loser = None;

            for tuple in vote_counts.tuples() {
                let candidate = tuple.get_typed::<String>("candidate_id").unwrap().clone();
                let votes = tuple.get_typed::<i64>("votes").unwrap();

                if votes > max_votes {
                    max_votes = votes;
                    winner = Some(candidate.clone());
                }

                if votes < min_votes {
                    min_votes = votes;
                    loser = Some(candidate.clone());
                }
            }

            if max_votes > threshold as i64 {
                return Ok(winner);
            }

            // If everyone has the same number of votes, it's a tie (or only 1 candidate left)
            if max_votes == min_votes {
                return Ok(winner); // Tie break or last candidate standing
            }

            // 6. Eliminate the loser
            if let Some(loser_id) = loser {
                self.eliminated
                    .insert(tuple! { candidate_id: loser_id })
                    .unwrap();
            } else {
                break;
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranked_choice_voting() {
        let mut rcv = RankedChoiceVoting::new();

        // Voter 1: A > B > C
        rcv.ballots
            .insert(tuple! { voter_id: 1i64, candidate_id: "A", rank: 1i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 1i64, candidate_id: "B", rank: 2i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 1i64, candidate_id: "C", rank: 3i64 })
            .unwrap();

        // Voter 2: A > C > B
        rcv.ballots
            .insert(tuple! { voter_id: 2i64, candidate_id: "A", rank: 1i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 2i64, candidate_id: "C", rank: 2i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 2i64, candidate_id: "B", rank: 3i64 })
            .unwrap();

        // Voter 3: B > C > A
        rcv.ballots
            .insert(tuple! { voter_id: 3i64, candidate_id: "B", rank: 1i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 3i64, candidate_id: "C", rank: 2i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 3i64, candidate_id: "A", rank: 3i64 })
            .unwrap();

        // Voter 4: C > B > A
        rcv.ballots
            .insert(tuple! { voter_id: 4i64, candidate_id: "C", rank: 1i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 4i64, candidate_id: "B", rank: 2i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 4i64, candidate_id: "A", rank: 3i64 })
            .unwrap();

        // Voter 5: C > A > B
        rcv.ballots
            .insert(tuple! { voter_id: 5i64, candidate_id: "C", rank: 1i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 5i64, candidate_id: "A", rank: 2i64 })
            .unwrap();
        rcv.ballots
            .insert(tuple! { voter_id: 5i64, candidate_id: "B", rank: 3i64 })
            .unwrap();

        // Initial 1st place votes:
        // A: 2 (Voter 1, 2)
        // B: 1 (Voter 3)
        // C: 2 (Voter 4, 5)
        // Total: 5. Threshold: 2.
        // No one > 2.
        // Min votes: B (1). B is eliminated.
        // B's votes (Voter 3) transfer to their next choice: C.
        // Round 2:
        // A: 2 (Voter 1, 2)
        // C: 3 (Voter 4, 5, plus Voter 3)
        // C has 3 votes. 3 > 2. C wins!

        let winner = rcv.evaluate().unwrap();
        assert_eq!(winner.unwrap(), "C");

        // Check that B is in the eliminated relation
        assert_eq!(rcv.eliminated.cardinality(), 1);
        let eliminated_tuple = rcv.eliminated.tuples().next().unwrap();
        assert_eq!(
            eliminated_tuple
                .get_typed::<String>("candidate_id")
                .unwrap(),
            "B"
        );
    }
}
