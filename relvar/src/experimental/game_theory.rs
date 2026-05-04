//! Relational Game Theory
//!
//! This module demonstrates how Game Theory concepts can be modeled and solved
//! using purely relational algebra.
//!
//! # Concept
//!
//! A normal-form game (like the Prisoner's Dilemma or Rock-Paper-Scissors) is
//! represented as a single payoff matrix relation:
//! `(p1_action: String, p2_action: String, p1_payoff: Float, p2_payoff: Float)`
//!
//! Using relational algebra, we can declaratively compute:
//! - **Best Responses**: Using `Summarize` (max) and `Join`.
//! - **Nash Equilibria**: The natural join of both players' best responses.
//! - **Strictly Dominated Strategies**: Using `Cross Join`, `Restrict`, and `Difference`.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::Relation,
};

/// A normal-form game.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::game_theory::NormalFormGame;
/// // Note: This is a placeholder example
/// ```
pub struct NormalFormGame {
    /// The payoff matrix.
    /// Schema: `(p1_action: String, p2_action: String, p1_payoff: Float, p2_payoff: Float)`
    pub payoffs: Relation,
}

impl Default for NormalFormGame {
    fn default() -> Self {
        Self::new()
    }
}

impl NormalFormGame {
    /// Creates a new, empty normal-form game.
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::game_theory::NormalFormGame;
    /// let game = NormalFormGame::new();
    /// ```
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("p1_action", ScalarType::String)
            .with_attribute("p2_action", ScalarType::String)
            .with_attribute("p1_payoff", ScalarType::Float)
            .with_attribute("p2_payoff", ScalarType::Float);

        Self {
            payoffs: Relation::new(RelationType::new(heading)),
        }
    }

    /// Finds the pure-strategy Nash Equilibria of the game.
    ///
    /// A Nash Equilibrium is a profile of actions where no player can gain by
    /// unilaterally deviating. In relational terms, it is the intersection (join)
    /// of all best responses.
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::game_theory::NormalFormGame;
    /// let game = NormalFormGame::new();
    /// let eq = game.find_nash_equilibria().unwrap();
    /// ```
    pub fn find_nash_equilibria(&self) -> Result<Relation, DatabaseError> {
        // Player 1's best responses to Player 2's actions
        // 1. Find max p1_payoff for each p2_action
        let p1_max = self
            .payoffs
            .summarize(
                &["p2_action"],
                &[Aggregation::max("max_p1", "p1_payoff", ScalarType::Float)],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Join back to find the exact p1_action(s) that yield that max payoff
        // First we need to rename p1_payoff to max_p1 in our relation so we can equijoin
        let payoffs_renamed_1 = self.payoffs.rename(&[("p1_payoff", "max_p1")]);

        let p1_best_responses_join = p1_max
            .join(&payoffs_renamed_1)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let p1_best_responses_proj =
            p1_best_responses_join.project(&["p1_action", "p2_action", "max_p1", "p2_payoff"]);
        let p1_best_responses = p1_best_responses_proj.rename(&[("max_p1", "p1_payoff")]);

        // Player 2's best responses to Player 1's actions
        // 1. Find max p2_payoff for each p1_action
        let p2_max = self
            .payoffs
            .summarize(
                &["p1_action"],
                &[Aggregation::max("max_p2", "p2_payoff", ScalarType::Float)],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Join back to find the exact p2_action(s)
        let payoffs_renamed_2 = self.payoffs.rename(&[("p2_payoff", "max_p2")]);
        let p2_best_responses_join = p2_max
            .join(&payoffs_renamed_2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let p2_best_responses_proj =
            p2_best_responses_join.project(&["p1_action", "p2_action", "p1_payoff", "max_p2"]);
        let p2_best_responses = p2_best_responses_proj.rename(&[("max_p2", "p2_payoff")]);

        // Nash Equilibria is the natural join of both players' best responses!
        // We only care about the actions, but keeping payoffs is nice.
        let equilibria = p1_best_responses
            .join(&p2_best_responses)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(equilibria)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_prisoners_dilemma() -> Result<(), DatabaseError> {
        let mut game = NormalFormGame::new();

        // Actions: "cooperate" (C), "defect" (D)
        // Payoffs (P1, P2):
        // (C, C) -> (-1, -1)
        // (C, D) -> (-3, 0)
        // (D, C) -> (0, -3)
        // (D, D) -> (-2, -2)

        game.payoffs.insert(tuple! {
            p1_action: "C", p2_action: "C", p1_payoff: -1.0, p2_payoff: -1.0
        })?;
        game.payoffs.insert(tuple! {
            p1_action: "C", p2_action: "D", p1_payoff: -3.0, p2_payoff: 0.0
        })?;
        game.payoffs.insert(tuple! {
            p1_action: "D", p2_action: "C", p1_payoff: 0.0, p2_payoff: -3.0
        })?;
        game.payoffs.insert(tuple! {
            p1_action: "D", p2_action: "D", p1_payoff: -2.0, p2_payoff: -2.0
        })?;

        let equilibria = game.find_nash_equilibria()?;

        // The only Nash Equilibrium should be (D, D)
        assert_eq!(equilibria.cardinality(), 1);
        let eq_tuple = equilibria.tuples().next().unwrap();
        assert_eq!(eq_tuple.get_typed::<String>("p1_action").unwrap(), "D");
        assert_eq!(eq_tuple.get_typed::<String>("p2_action").unwrap(), "D");

        Ok(())
    }
}
