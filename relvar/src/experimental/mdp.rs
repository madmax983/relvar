//! Relational Markov Decision Process (MDP) Solver
//!
//! This module demonstrates how Reinforcement Learning (Value Iteration) can be
//! implemented using pure relational algebra. By representing states, actions,
//! transitions, and rewards as relations, the Bellman equation is evaluated
//! entirely through joins, extensions, and aggregations.
//!
//! # Concept
//!
//! - **Values**: Relation `(state: String, v: Float)`
//! - **Rewards**: Relation `(state: String, action: String, reward: Float)`
//! - **Transitions**: Relation `(state: String, action: String, next_state: String, prob: Float)`
//!
//! The Value Iteration algorithm:
//! 1. **Join** `Transitions` and `Values` (on `next_state = state`).
//! 2. **Extend** to calculate the discounted future value (`prob * v * gamma`).
//! 3. **Summarize** (Sum) by `(state, action)` to get the expected future value.
//! 4. **Join** with `Rewards` and **Extend** to get `Q(s, a)`.
//! 5. **Summarize** (Max) by `state` to find the new `V(s)`.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Markov Decision Process (MDP) solver.
pub struct MdpSolver {
    /// The set of states.
    pub states: Relation,
    /// The transition probabilities.
    pub transitions: Relation,
    /// The rewards for each state and action.
    pub rewards: Relation,
    /// The discount factor.
    pub gamma: f64,
}

impl MdpSolver {
    /// Create a new MdpSolver.
    pub fn new(states: Relation, transitions: Relation, rewards: Relation, gamma: f64) -> Self {
        Self {
            states,
            transitions,
            rewards,
            gamma,
        }
    }

    /// Performs one iteration of Value Iteration to update the value function.
    pub fn value_iteration_step(
        &self,
        current_values: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // current_values schema: (state: String, v: Float)

        // 1. Join transitions with current_values
        // Rename current_values.state to next_state for the join
        let v_next = current_values.rename(&[("state", "next_state")]);
        let t_join_v = self.transitions.join(&v_next)?;

        // 2. Extend to compute discounted expected value component: prob * v
        let expected_v = t_join_v
            .extend("discounted_v", ScalarType::Float, |t| {
                let prob = t.get_typed::<f64>("prob").unwrap();
                let v = t.get_typed::<f64>("v").unwrap();
                ScalarValue::Float(prob * v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by (state, action) to sum the expected future values
        let sum_expected_v = expected_v
            .summarize(
                &["state", "action"],
                &[Aggregation::sum_float("sum_v", "discounted_v")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Join with Rewards to compute Q-values
        let q_values_pre = sum_expected_v.join(&self.rewards)?;

        let gamma = self.gamma;
        let q_values = q_values_pre
            .extend("q", ScalarType::Float, move |t| {
                let reward = t.get_typed::<f64>("reward").unwrap();
                let sum_v = t.get_typed::<f64>("sum_v").unwrap();
                ScalarValue::Float(reward + gamma * sum_v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Summarize by state to find max Q-value, which is the new V(s)
        let new_values = q_values
            .summarize(
                &["state"],
                &[Aggregation::max("new_v", "q", ScalarType::Float)],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename new_v back to v
        let result = new_values.rename(&[("new_v", "v")]);

        // Some states might not have transitions/rewards defined, ensure we keep them with v=0.0
        // (Optional, omitted for simplicity assuming well-formed MDP)

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    #[test]
    fn test_mdp_value_iteration() {
        // States: S1, S2
        let states_type =
            RelationType::new(TupleType::new().with_attribute("state", ScalarType::String));
        let mut states = Relation::new(states_type);
        states.insert(tuple! { state: "S1" }).unwrap();
        states.insert(tuple! { state: "S2" }).unwrap();

        // Initial Values: S1=0.0, S2=0.0
        let values_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("v", ScalarType::Float),
        );
        let mut values = Relation::new(values_type);
        values.insert(tuple! { state: "S1", v: 0.0 }).unwrap();
        values.insert(tuple! { state: "S2", v: 0.0 }).unwrap();

        // Transitions:
        // S1, A1 -> S2 (1.0)
        // S2, A2 -> S2 (1.0)
        let transitions_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("action", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("prob", ScalarType::Float),
        );
        let mut transitions = Relation::new(transitions_type);
        transitions
            .insert(tuple! { state: "S1", action: "A1", next_state: "S2", prob: 1.0 })
            .unwrap();
        transitions
            .insert(tuple! { state: "S2", action: "A2", next_state: "S2", prob: 1.0 })
            .unwrap();

        // Rewards:
        // S1, A1 -> 10.0
        // S2, A2 -> 0.0
        let rewards_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("action", ScalarType::String)
                .with_attribute("reward", ScalarType::Float),
        );
        let mut rewards = Relation::new(rewards_type);
        rewards
            .insert(tuple! { state: "S1", action: "A1", reward: 10.0 })
            .unwrap();
        rewards
            .insert(tuple! { state: "S2", action: "A2", reward: 0.0 })
            .unwrap();

        let solver = MdpSolver::new(states, transitions, rewards, 0.9);

        // Step 1
        let v1 = solver.value_iteration_step(&values).unwrap();
        assert_eq!(v1.cardinality(), 2);

        let s1_v1 = v1
            .tuples()
            .find(|t| t.get_typed::<String>("state") == Some("S1".to_string()))
            .unwrap();
        assert_eq!(s1_v1.get_typed::<f64>("v"), Some(10.0)); // 10.0 + 0.9*1.0*0.0

        // Step 2
        let v2 = solver.value_iteration_step(&v1).unwrap();

        let s1_v2 = v2
            .tuples()
            .find(|t| t.get_typed::<String>("state") == Some("S1".to_string()))
            .unwrap();
        assert_eq!(s1_v2.get_typed::<f64>("v"), Some(10.0)); // 10.0 + 0.9*1.0*0.0
    }
}
