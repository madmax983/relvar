//! Relational Q-Learning
//!
//! This module implements Tabular Q-Learning using purely relational algebra.
//! States, actions, and the Q-table are represented as relations, and the
//! Bellman equation is evaluated using relational operators like Join,
//! Extend, and Summarize.
//!
//! # Concept
//!
//! - **Q-Table**: Relation `(state: String, action: String, q_value: Float)`.
//! - **Transitions**: Relation `(state: String, action: String, reward: Float, next_state: String)`.
//!
//! The learning step computes the TD-target `R + gamma * max_a' Q(S', a')`
//! and updates the Q-table.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Tabular Q-Learning Model.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::q_learning::QLearning;
/// // Note: This is a placeholder example
/// ```
pub struct QLearning {
    /// The Q-table. Schema: (state: String, action: String, q_value: Float)
    pub q_table: Relation,
    /// The transition table. Schema: (state: String, action: String, reward: Float, next_state: String)
    pub transitions: Relation,
    /// The learning rate (alpha).
    pub alpha: f64,
    /// The discount factor (gamma).
    pub gamma: f64,
}

impl QLearning {
    /// Creates a new QLearning model.
    pub fn new(q_table: Relation, transitions: Relation, alpha: f64, gamma: f64) -> Self {
        Self {
            q_table,
            transitions,
            alpha,
            gamma,
        }
    }

    /// Performs one step of Q-value iteration over all transitions simultaneously.
    ///
    /// This evaluates the Bellman equation using relational algebra.
    pub fn step(&mut self) -> Result<(), DatabaseError> {
        // 1. Calculate the max Q-value for each next state.
        // First, rename the q_table's state to match next_state from transitions.
        let next_q = self.q_table.rename(&[("state", "next_state")]);

        // Summarize next_q by next_state to find the max q_value.
        let max_next_q = next_q
            .summarize(
                &["next_state"],
                &[Aggregation::max("max_q", "q_value", ScalarType::Float)],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Join transitions with max_next_q.
        // This gives us (state, action, reward, next_state, max_q) for each transition.
        // Some next states might not have any actions in the q_table yet.
        // We'll join, and for those that don't match, they just won't be updated yet
        // (or we could use an outer join, but this is simpler for now).
        let joined_transitions = self.transitions.join(&max_next_q)?;

        // 3. Join with current q_table to get current q_value.
        let current_q_join = joined_transitions.join(&self.q_table)?;

        let alpha = self.alpha;
        let gamma = self.gamma;

        // 4. Compute the new Q-value using Extend.
        // new_q = (1 - alpha) * current_q + alpha * (reward + gamma * max_next_q)
        let updated_q_table = current_q_join
            .extend("new_q_value", ScalarType::Float, move |t: &Tuple| {
                let current_q = t.get_typed::<f64>("q_value").unwrap();
                let reward = t.get_typed::<f64>("reward").unwrap();
                let max_q = t.get_typed::<f64>("max_q").unwrap();

                let target = reward + gamma * max_q;
                let new_q = current_q + alpha * (target - current_q);
                ScalarValue::Float(new_q)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Project back to the original Q-table schema.
        let new_q_projected = updated_q_table
            .project(&["state", "action", "new_q_value"])
            .rename(&[("new_q_value", "q_value")]);

        // 6. Update the main Q-table.
        // We need to overwrite the updated values in the original table.
        // First, find the entries in the old table that were updated.
        let updated_keys = new_q_projected.project(&["state", "action"]);

        let unmodified_q_table = self.q_table.semidifference(&updated_keys);

        self.q_table = unmodified_q_table
            .union(&new_q_projected)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }

    /// Runs the learning process for a given number of iterations.
    pub fn train(&mut self, iterations: usize) -> Result<(), DatabaseError> {
        for _ in 0..iterations {
            self.step()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::RelationType;
    use relvar_core::types::TupleType;

    #[test]
    fn test_q_learning_step() {
        let q_table_heading = TupleType::new()
            .with_attribute("state".to_string(), ScalarType::String)
            .with_attribute("action".to_string(), ScalarType::String)
            .with_attribute("q_value".to_string(), ScalarType::Float);
        let mut q_table = Relation::new(RelationType::new(q_table_heading));

        // Initial Q-values
        q_table
            .insert(tuple! { state: "A", action: "up", q_value: 0.0f64 })
            .unwrap();
        q_table
            .insert(tuple! { state: "B", action: "down", q_value: 0.5f64 })
            .unwrap();
        q_table
            .insert(tuple! { state: "B", action: "up", q_value: 1.0f64 })
            .unwrap(); // max_q for B is 1.0

        let transitions_heading = TupleType::new()
            .with_attribute("state".to_string(), ScalarType::String)
            .with_attribute("action".to_string(), ScalarType::String)
            .with_attribute("reward".to_string(), ScalarType::Float)
            .with_attribute("next_state".to_string(), ScalarType::String);
        let mut transitions = Relation::new(RelationType::new(transitions_heading));

        // Transition: A, up -> B, reward: 1.0
        transitions
            .insert(tuple! { state: "A", action: "up", reward: 1.0f64, next_state: "B" })
            .unwrap();

        let mut model = QLearning::new(q_table, transitions, 0.5, 0.9);

        // Run one step
        model.step().unwrap();

        // Let's calculate the expected new Q-value for (A, up):
        // current_q = 0.0
        // reward = 1.0
        // max_next_q = 1.0 (since max(0.5, 1.0) = 1.0)
        // target = 1.0 + 0.9 * 1.0 = 1.9
        // new_q = 0.0 + 0.5 * (1.9 - 0.0) = 0.95

        let updated_tuple = model
            .q_table
            .restrict(|t| {
                t.get_typed::<String>("state").unwrap() == "A"
                    && t.get_typed::<String>("action").unwrap() == "up"
            })
            .tuples()
            .next()
            .unwrap()
            .clone();

        let new_q = updated_tuple.get_typed::<f64>("q_value").unwrap();

        // Assert within some epsilon due to float math
        assert!((new_q - 0.95).abs() < 1e-6, "Expected 0.95, got {}", new_q);
    }
}
