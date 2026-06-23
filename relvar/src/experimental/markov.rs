//! Relational Markov Chain
//!
//! This module demonstrates how Markov Chains can be evaluated using purely
//! relational algebra. The state distribution and transition matrix are both
//! represented as relations. The next state distribution is computed via
//! Join, Extend, and Summarize.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Markov Chain.
/// # Examples
///
/// ```
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar::experimental::MarkovChain;
/// // Note: This is a placeholder example
/// ```
pub struct MarkovChain {
    /// The state distribution. Schema: (state: String, probability: Float)
    pub state_distribution: Relation,
    /// The transition probabilities. Schema: (from_state: String, to_state: String, transition_prob: Float)
    pub transitions: Relation,
}

impl MarkovChain {
    /// Creates a new Relational Markov Chain.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, ScalarType, TupleType};
    /// use relvar_core::values::Relation;
    /// use relvar::experimental::MarkovChain;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(state_distribution: Relation, transitions: Relation) -> Self {
        Self {
            state_distribution,
            transitions,
        }
    }

    /// Computes the next state distribution.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, ScalarType, TupleType};
    /// use relvar_core::values::Relation;
    /// use relvar::experimental::MarkovChain;
    /// // Note: This is a placeholder example
    /// ```
    pub fn next_step(&mut self) -> Result<(), DatabaseError> {
        // 1. Rename attributes to prepare for join
        let current_dist = self
            .state_distribution
            .rename(&[("state", "from_state"), ("probability", "current_prob")]);

        // 2. Join current distribution with transitions
        let joined = current_dist.join(&self.transitions)?;

        // 3. Compute joint probability
        let joint_prob = joined
            .extend("joint_prob", ScalarType::Float, |t: &Tuple| {
                let current = t.get_typed::<f64>("current_prob").unwrap();
                let trans = t.get_typed::<f64>("transition_prob").unwrap();
                ScalarValue::Float(current * trans)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize (aggregate) probabilities by to_state
        let next_dist_raw = joint_prob
            .summarize(
                &["to_state"],
                &[Aggregation::sum_float("new_prob", "joint_prob")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Rename back to standard schema
        self.state_distribution =
            next_dist_raw.rename(&[("to_state", "state"), ("new_prob", "probability")]);

        Ok(())
    }

    /// Runs the Markov Chain for a specified number of steps.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, ScalarType, TupleType};
    /// use relvar_core::values::Relation;
    /// use relvar::experimental::MarkovChain;
    /// // Note: This is a placeholder example
    /// ```
    pub fn run(&mut self, steps: usize) -> Result<(), DatabaseError> {
        for _ in 0..steps {
            self.next_step()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_markov_chain_weather() {
        let dist_heading = TupleType::new()
            .with_attribute("state".to_string(), ScalarType::String)
            .with_attribute("probability".to_string(), ScalarType::Float);
        let mut dist = Relation::new(RelationType::new(dist_heading));
        // Initially 100% Sunny
        dist.insert(tuple! { state: "Sunny", probability: 1.0f64 })
            .unwrap();
        dist.insert(tuple! { state: "Rainy", probability: 0.0f64 })
            .unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("from_state".to_string(), ScalarType::String)
            .with_attribute("to_state".to_string(), ScalarType::String)
            .with_attribute("transition_prob".to_string(), ScalarType::Float);
        let mut trans = Relation::new(RelationType::new(trans_heading));
        // Sunny -> Sunny: 0.9, Sunny -> Rainy: 0.1
        trans
            .insert(tuple! { from_state: "Sunny", to_state: "Sunny", transition_prob: 0.9f64 })
            .unwrap();
        trans
            .insert(tuple! { from_state: "Sunny", to_state: "Rainy", transition_prob: 0.1f64 })
            .unwrap();
        // Rainy -> Sunny: 0.5, Rainy -> Rainy: 0.5
        trans
            .insert(tuple! { from_state: "Rainy", to_state: "Sunny", transition_prob: 0.5f64 })
            .unwrap();
        trans
            .insert(tuple! { from_state: "Rainy", to_state: "Rainy", transition_prob: 0.5f64 })
            .unwrap();

        let mut mc = MarkovChain::new(dist, trans);

        mc.next_step().unwrap();

        // After 1 step: Sunny 0.9, Rainy 0.1
        let sunny_prob_1 = mc
            .state_distribution
            .tuples()
            .find(|t| t.get_typed::<String>("state").unwrap() == "Sunny")
            .unwrap()
            .get_typed::<f64>("probability")
            .unwrap();
        let rainy_prob_1 = mc
            .state_distribution
            .tuples()
            .find(|t| t.get_typed::<String>("state").unwrap() == "Rainy")
            .unwrap()
            .get_typed::<f64>("probability")
            .unwrap();

        assert!((sunny_prob_1 - 0.9).abs() < 1e-6);
        assert!((rainy_prob_1 - 0.1).abs() < 1e-6);

        // Run many steps to approach stationary distribution
        mc.run(50).unwrap();

        // Stationary dist: P(Sunny) = 5/6 (0.8333), P(Rainy) = 1/6 (0.1666)
        let sunny_prob_inf = mc
            .state_distribution
            .tuples()
            .find(|t| t.get_typed::<String>("state").unwrap() == "Sunny")
            .unwrap()
            .get_typed::<f64>("probability")
            .unwrap();
        let rainy_prob_inf = mc
            .state_distribution
            .tuples()
            .find(|t| t.get_typed::<String>("state").unwrap() == "Rainy")
            .unwrap()
            .get_typed::<f64>("probability")
            .unwrap();

        assert!((sunny_prob_inf - (5.0 / 6.0)).abs() < 1e-4);
        assert!((rainy_prob_inf - (1.0 / 6.0)).abs() < 1e-4);
    }
}
