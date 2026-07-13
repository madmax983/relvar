use relvar_core::algebra::Aggregation;
use relvar_core::{DatabaseError, Relation, ScalarType, ScalarValue};

/// A Q-Learning Agent implemented purely with relational algebra.
pub struct QLearningAgent {
    /// The Q-table, a relation with attributes: (state: Int, action: Int, q_value: Float)
    pub q_table: Relation,
    /// Learning rate (0.0 to 1.0)
    pub alpha: f64,
    /// Discount factor (0.0 to 1.0)
    pub gamma: f64,
}

impl QLearningAgent {
    /// Creates a new Q-Learning agent.
    pub fn new(q_table: Relation, alpha: f64, gamma: f64) -> Self {
        Self {
            q_table,
            alpha,
            gamma,
        }
    }

    /// Updates the Q-table using a batch of experiences.
    /// `experiences` must have schema: (state: Int, action: Int, reward: Float, next_state: Int)
    pub fn learn(&self, experiences: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Find max Q(s', a') for all states currently in the Q-table
        let max_q_per_state = self
            .q_table
            .summarize(
                &["state"],
                &[Aggregation::max("max_q", "q_value", ScalarType::Float)],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename state -> next_state to match with experiences relation
        let max_q_for_next = max_q_per_state.rename(&[("state", "next_state")]);

        // 2. Join experiences with max_q_for_next
        let joined_experiences = experiences.join(&max_q_for_next)?;

        // Find experiences that transition to a state not in the Q-table (e.g., terminal states or unseen states)
        let exp_next_states = experiences.project(&["next_state"]);
        let known_next_states = max_q_for_next.project(&["next_state"]);

        let missing_next_states = exp_next_states
            .difference(&known_next_states)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Assign max_q = 0.0 for those missing states
        let missing_experiences = experiences
            .join(&missing_next_states)?
            .extend("max_q", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_experiences = joined_experiences
            .union(&missing_experiences)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Join with current Q-table to get the current q_value for the experienced (state, action) pairs
        let exp_with_q = all_experiences.join(&self.q_table)?;

        let alpha = self.alpha;
        let gamma = self.gamma;

        // 4. Compute the new Q-value using the Bellman equation
        let updated_q_table = exp_with_q
            .extend("new_q", ScalarType::Float, move |t| {
                let reward = t.get_typed::<f64>("reward").unwrap();
                let max_q = t.get_typed::<f64>("max_q").unwrap();
                let current_q = t.get_typed::<f64>("q_value").unwrap();

                let target = reward + gamma * max_q;
                let new_q = current_q + alpha * (target - current_q);

                ScalarValue::Float(new_q)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Project out the intermediate columns and rename to match Q-table schema
        let new_updated_q = updated_q_table
            .project(&["state", "action", "new_q"])
            .rename(&[("new_q", "q_value")]);

        // 6. Merge the updated Q-values with the untouched parts of the Q-table
        let updated_keys = new_updated_q.project(&["state", "action"]);
        let q_to_remove = self.q_table.join(&updated_keys)?;

        let untouched_q_table = self
            .q_table
            .difference(&q_to_remove)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let final_q_table = untouched_q_table
            .union(&new_updated_q)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(final_q_table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_q_learning() {
        let q_table_schema = TupleType::new()
            .with_attribute("state", ScalarType::Int)
            .with_attribute("action", ScalarType::Int)
            .with_attribute("q_value", ScalarType::Float);
        let mut q_table = Relation::new(RelationType::new(q_table_schema));

        // Initial Q-values (all 0)
        q_table
            .insert(tuple! { state: 1i64, action: 1i64, q_value: 0.0f64 })
            .unwrap();
        q_table
            .insert(tuple! { state: 1i64, action: 2i64, q_value: 0.0f64 })
            .unwrap();
        q_table
            .insert(tuple! { state: 2i64, action: 1i64, q_value: 0.0f64 })
            .unwrap();

        let agent = QLearningAgent::new(q_table.clone(), 0.1, 0.9);

        let exp_schema = TupleType::new()
            .with_attribute("state", ScalarType::Int)
            .with_attribute("action", ScalarType::Int)
            .with_attribute("reward", ScalarType::Float)
            .with_attribute("next_state", ScalarType::Int);
        let mut experiences = Relation::new(RelationType::new(exp_schema));

        // Experience: took action 1 in state 1, got reward 10.0, ended up in state 2
        experiences
            .insert(tuple! { state: 1i64, action: 1i64, reward: 10.0f64, next_state: 2i64 })
            .unwrap();

        // Experience: took action 1 in state 2, got reward -5.0, ended up in state 3 (terminal/unseen)
        experiences
            .insert(tuple! { state: 2i64, action: 1i64, reward: -5.0f64, next_state: 3i64 })
            .unwrap();

        let new_q_table = agent.learn(&experiences).unwrap();

        // Check new Q-values
        let q_1_1 = new_q_table
            .tuples()
            .find(|t| {
                t.get_typed::<i64>("state") == Some(1) && t.get_typed::<i64>("action") == Some(1)
            })
            .unwrap()
            .get_typed::<f64>("q_value")
            .unwrap();

        // Q(1,1) = 0 + 0.1 * (10.0 + 0.9 * max_a Q(2, a) - 0) = 0.1 * (10.0 + 0 - 0) = 1.0
        assert_eq!(q_1_1, 1.0);

        let q_2_1 = new_q_table
            .tuples()
            .find(|t| {
                t.get_typed::<i64>("state") == Some(2) && t.get_typed::<i64>("action") == Some(1)
            })
            .unwrap()
            .get_typed::<f64>("q_value")
            .unwrap();

        // Q(2,1) = 0 + 0.1 * (-5.0 + 0.9 * max_a Q(3, a) - 0) = 0.1 * (-5.0 + 0 - 0) = -0.5
        assert_eq!(q_2_1, -0.5);

        // Untouched should still be 0.0
        let q_1_2 = new_q_table
            .tuples()
            .find(|t| {
                t.get_typed::<i64>("state") == Some(1) && t.get_typed::<i64>("action") == Some(2)
            })
            .unwrap()
            .get_typed::<f64>("q_value")
            .unwrap();
        assert_eq!(q_1_2, 0.0);
    }
}
