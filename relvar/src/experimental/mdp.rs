use relvar_core::algebra::Aggregation;
use relvar_core::{DatabaseError, Relation, ScalarType, ScalarValue};

/// A Relational Markov Decision Process (MDP) solver.
///
/// This engine uses purely relational algebra to perform Value Iteration,
/// a dynamic programming algorithm used in Reinforcement Learning to find
/// the optimal state-value function and optimal policy.
pub struct MarkovDecisionProcess {
    /// Relation of states: (state: String, reward: Float)
    pub states: Relation,
    /// Relation of transitions: (state: String, action: String, next_state: String, probability: Float)
    pub transitions: Relation,
    /// Discount factor (gamma) in [0, 1]
    pub discount_factor: f64,
}

impl MarkovDecisionProcess {
    /// Creates a new Markov Decision Process.
    pub fn new(states: Relation, transitions: Relation, discount_factor: f64) -> Self {
        Self {
            states,
            transitions,
            discount_factor,
        }
    }

    /// Performs Value Iteration to compute the optimal state values.
    /// Returns a relation: (state: String, value: Float)
    pub fn value_iteration(&self, iterations: usize) -> Result<Relation, DatabaseError> {
        // Initialize values: V(s) = 0 for all states
        let mut values = self
            .states
            .project(&["state"])
            .extend("value", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        for _ in 0..iterations {
            values = self.value_iteration_step(&values)?;
        }

        Ok(values)
    }

    fn value_iteration_step(&self, values: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Join transitions with current values of next_states
        let values_renamed = values.rename(&[("state", "next_state")]);
        let t_joined = self.transitions.join(&values_renamed)?;

        // 2. Compute probability * value
        let expected = t_joined
            .extend("expected_val", ScalarType::Float, |t| {
                let prob = t.get_typed::<f64>("probability").unwrap();
                let val = t.get_typed::<f64>("value").unwrap();
                ScalarValue::Float(prob * val)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Sum expected values over next_states for each (state, action) pair
        let action_values = expected
            .summarize(
                &["state", "action"],
                &[Aggregation::sum_float("action_value", "expected_val")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Maximize over actions for each state
        let max_action_values = action_values
            .summarize(
                &["state"],
                &[Aggregation::max(
                    "max_action_value",
                    "action_value",
                    ScalarType::Float,
                )],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Join with states to add immediate rewards
        let state_rewards = self.states.join(&max_action_values)?;

        // 6. Compute new value = reward + discount_factor * max_action_value
        let gamma = self.discount_factor;
        let new_values = state_rewards
            .extend("new_value", ScalarType::Float, move |t| {
                let r = t.get_typed::<f64>("reward").unwrap();
                let max_v = t.get_typed::<f64>("max_action_value").unwrap();
                ScalarValue::Float(r + gamma * max_v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 7. Project back to (state, value) and return
        let next_values = new_values
            .project(&["state", "new_value"])
            .rename(&[("new_value", "value")]);

        Ok(next_values)
    }

    /// Extracts the optimal policy given the computed values.
    /// Returns a relation: (state: String, optimal_action: String)
    pub fn extract_policy(&self, values: &Relation) -> Result<Relation, DatabaseError> {
        let values_renamed = values.rename(&[("state", "next_state")]);
        let t_joined = self.transitions.join(&values_renamed)?;

        let expected = t_joined
            .extend("expected_val", ScalarType::Float, |t| {
                let prob = t.get_typed::<f64>("probability").unwrap();
                let val = t.get_typed::<f64>("value").unwrap();
                ScalarValue::Float(prob * val)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let action_values = expected
            .summarize(
                &["state", "action"],
                &[Aggregation::sum_float("action_value", "expected_val")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let max_action_values = action_values
            .summarize(
                &["state"],
                &[Aggregation::max(
                    "max_action_value",
                    "action_value",
                    ScalarType::Float,
                )],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // To find the argmax action, join back action_values with max_action_values
        // where action_value == max_action_value
        let best_actions = action_values.join(&max_action_values)?.restrict(|t| {
            let av = t.get_typed::<f64>("action_value").unwrap();
            let mav = t.get_typed::<f64>("max_action_value").unwrap();
            (av - mav).abs() < 1e-6
        });

        let policy = best_actions
            .project(&["state", "action"])
            .rename(&[("action", "optimal_action")]);

        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_mdp_value_iteration() {
        let states_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("reward", ScalarType::Float),
        );
        let mut states = Relation::new(states_type);
        states
            .insert(tuple! { state: "s1", reward: 0.0f64 })
            .unwrap();
        states
            .insert(tuple! { state: "s2", reward: 1.0f64 })
            .unwrap();
        states
            .insert(tuple! { state: "s3", reward: 10.0f64 })
            .unwrap(); // Goal state

        let transitions_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("action", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("probability", ScalarType::Float),
        );
        let mut transitions = Relation::new(transitions_type);
        // From s1, can go to s2 or stay
        transitions
            .insert(tuple! { state: "s1", action: "move", next_state: "s2", probability: 1.0f64 })
            .unwrap();
        transitions
            .insert(tuple! { state: "s1", action: "stay", next_state: "s1", probability: 1.0f64 })
            .unwrap();

        // From s2, can go to s3 or stay
        transitions
            .insert(tuple! { state: "s2", action: "move", next_state: "s3", probability: 1.0f64 })
            .unwrap();
        transitions
            .insert(tuple! { state: "s2", action: "stay", next_state: "s2", probability: 1.0f64 })
            .unwrap();

        // From s3, terminal
        transitions
            .insert(tuple! { state: "s3", action: "stay", next_state: "s3", probability: 1.0f64 })
            .unwrap();

        let mdp = MarkovDecisionProcess::new(states, transitions, 0.9);

        let values = mdp.value_iteration(10).unwrap();
        assert_eq!(values.cardinality(), 3);

        let policy = mdp.extract_policy(&values).unwrap();
        assert!(policy.cardinality() >= 3);

        let s1_policy = policy
            .tuples()
            .find(|t| t.get_typed::<String>("state").unwrap() == "s1")
            .unwrap();
        assert_eq!(
            s1_policy.get_typed::<String>("optimal_action").unwrap(),
            "move"
        );

        let s2_policy = policy
            .tuples()
            .find(|t| t.get_typed::<String>("state").unwrap() == "s2")
            .unwrap();
        assert_eq!(
            s2_policy.get_typed::<String>("optimal_action").unwrap(),
            "move"
        );
    }
}
