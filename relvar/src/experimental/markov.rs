//! Relational Markov Chain
//!
//! This module implements a Markov Chain trained purely using relational algebra primitives.
//! It connects concepts from Graph Analytics (transitions/edges) and AutoML (probability counting).
//!
//! # Concept
//!
//! A Markov Chain predicts the next state based entirely on the current state.
//! It requires transition probabilities between states.
//! We can compute these using relational algebra on a sequence dataset:
//! - **Extend** the sequence relation to shift the order attribute.
//! - **Rename** attributes to distinguish between the 'from' and 'to' states.
//! - **Join** the original relation and the shifted relation to extract state transitions `(from_state, to_state)`.
//! - **Summarize** to count frequencies of `(from_state, to_state)`.
//! - **Summarize** again to get total outgoing transitions per state.
//! - **Join** the counts and **Extend** to compute the transition probability `P(to_state | from_state)`.
//!
//! # Example
//!
//! ```
//! use relvar_core::values::{Relation, ScalarValue};
//! use relvar_core::types::{RelationType, ScalarType, TupleType};
//! use relvar_core::tuple;
//! use relvar::experimental::markov::MarkovChain;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Create Sequence Data Schema (seq_id, step, weather)
//! let heading = TupleType::new()
//!     .with_attribute("seq_id", ScalarType::Int)
//!     .with_attribute("step", ScalarType::Int)
//!     .with_attribute("weather", ScalarType::String);
//!
//! let mut log = Relation::new(RelationType::new(heading));
//!
//! // 2. Insert Sequence Data
//! // Sequence 1: Sunny -> Sunny -> Rain -> Sunny
//! log.insert(tuple! { seq_id: 1i64, step: 1i64, weather: "Sunny" })?;
//! log.insert(tuple! { seq_id: 1i64, step: 2i64, weather: "Sunny" })?;
//! log.insert(tuple! { seq_id: 1i64, step: 3i64, weather: "Rain" })?;
//! log.insert(tuple! { seq_id: 1i64, step: 4i64, weather: "Sunny" })?;
//!
//! // 3. Train Model
//! let mut model = MarkovChain::new();
//! model.train(&log, "seq_id", "step", "weather")?;
//!
//! // 4. Generate Sequence
//! let start_state = ScalarValue::String("Sunny".to_string());
//! let sequence = model.generate(start_state, 3, 42); // Using fixed seed 42
//!
//! assert_eq!(sequence.len(), 4); // Start state + 3 generated states
//! # Ok(())
//! # }
//! ```

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue};
use std::collections::BTreeMap;

/// A Markov Chain model that generates state sequences based on transition probabilities
/// learned from a relational dataset.
pub struct MarkovChain {
    /// Probabilities of transitioning from one state to another.
    /// Maps from `from_state` -> list of `(to_state, probability)`.
    transitions: BTreeMap<ScalarValue, Vec<(ScalarValue, f64)>>,
}

impl Default for MarkovChain {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkovChain {
    /// Creates a new empty Markov Chain.
    pub fn new() -> Self {
        Self {
            transitions: BTreeMap::new(),
        }
    }

    /// Trains the Markov Chain on a relation containing state sequences.
    ///
    /// # Arguments
    ///
    /// * `relation` - The relation containing sequence data.
    /// * `seq_id_attr` - Attribute identifying distinct sequences (so transitions don't jump across sequences).
    /// * `order_attr` - Attribute specifying the order/step of the state in the sequence (must be `Int`).
    /// * `state_attr` - Attribute representing the state value.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AttributeNotFound` if specified attributes are missing.
    /// Returns `DatabaseError::AlgebraError` if internal algebra operations fail.
    pub fn train(
        &mut self,
        relation: &Relation,
        seq_id_attr: &str,
        order_attr: &str,
        state_attr: &str,
    ) -> Result<(), DatabaseError> {
        let heading = relation.relation_type().heading();

        if !heading.has_attribute(seq_id_attr) {
            return Err(DatabaseError::AttributeNotFound(
                seq_id_attr.to_string(),
                "<input_relation>".to_string(),
            ));
        }
        if !heading.has_attribute(order_attr) {
            return Err(DatabaseError::AttributeNotFound(
                order_attr.to_string(),
                "<input_relation>".to_string(),
            ));
        }
        if !heading.has_attribute(state_attr) {
            return Err(DatabaseError::AttributeNotFound(
                state_attr.to_string(),
                "<input_relation>".to_string(),
            ));
        }

        // We want to join relation T1 with relation T2 (both are copies of `relation`)
        // Condition: T1.seq_id = T2.seq_id AND T2.order = T1.order + 1
        // This is equivalent to finding consecutive states in a sequence.

        // 1. Rename T1 attributes to distinguish `from` states
        let from_state_attr = format!("from_{}", state_attr);
        let t1_rename_map = vec![(state_attr, from_state_attr.as_str())];
        let t1 = relation.rename(&t1_rename_map);

        // 2. Extend T2 with `shifted_order` = `order - 1`
        // We will join T1.order with T2.shifted_order
        // Then rename T2 attributes to distinguish `to` states
        let to_state_attr = format!("to_{}", state_attr);
        let t2 = relation
            .extend("shifted_order", ScalarType::Int, move |t| {
                let current_order = t.get_typed::<i64>(order_attr).unwrap();
                ScalarValue::Int(current_order - 1)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename T2's state to 'to_state', and 'shifted_order' to 'order' so we can natural join.
        // Also keep seq_id as is so it naturally joins.
        let t2_rename_map = vec![
            (state_attr, to_state_attr.as_str()),
            ("shifted_order", order_attr),
        ];

        // Project out T2's original order_attr before renaming so we don't have conflicts.
        // We need: seq_id, state_attr (to rename), shifted_order (to rename to order).
        let t2_projected = t2.project(&[seq_id_attr, state_attr, "shifted_order"]);
        let t2_renamed = t2_projected.rename(&t2_rename_map);

        // 3. Join `t1` and `t2_renamed`
        // Natural Join will happen on: `seq_id` and `order` (which is T1.order and T2.shifted_order)
        let transitions_rel = t1.join(&t2_renamed)?;

        // If the relation is empty, we just clear transitions and return
        if transitions_rel.is_empty() {
            self.transitions.clear();
            return Ok(());
        }

        // 4. Count frequencies of (from_state, to_state) pairs
        let pair_counts = transitions_rel
            .summarize(
                &[&from_state_attr, &to_state_attr],
                &[Aggregation::count("pair_count")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Count total outgoing transitions from from_state
        let total_counts = pair_counts
            .summarize(
                &[&from_state_attr],
                &[Aggregation::sum("total_count", "pair_count")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Join to calculate probabilities
        let probabilities = pair_counts.join(&total_counts)?;

        let final_probs = probabilities
            .extend("probability", ScalarType::Float, move |t| {
                let pair = t.get_typed::<i64>("pair_count").unwrap() as f64;
                let total = t.get_typed::<i64>("total_count").unwrap() as f64;
                ScalarValue::Float(pair / total)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Extract and build transition map
        self.transitions.clear();

        for tuple in final_probs.tuples() {
            let from_val = tuple.get(&from_state_attr).unwrap().clone();
            let to_val = tuple.get(&to_state_attr).unwrap().clone();
            let prob = tuple.get_typed::<f64>("probability").unwrap();

            self.transitions
                .entry(from_val)
                .or_default()
                .push((to_val, prob));
        }

        // Sort probabilities internally for deterministic random selection
        for targets in self.transitions.values_mut() {
            targets.sort_by(|a, b| a.0.cmp(&b.0));
        }

        Ok(())
    }

    /// Generates a sequence of states starting from `start_state`.
    ///
    /// # Arguments
    ///
    /// * `start_state` - The initial state.
    /// * `steps` - How many additional steps to generate.
    /// * `seed` - Random seed for deterministic generation.
    ///
    /// # Returns
    ///
    /// A vector of states, where the first element is `start_state`.
    /// If the chain reaches a state with no outgoing transitions, generation stops early.
    pub fn generate(&self, start_state: ScalarValue, steps: usize, seed: u64) -> Vec<ScalarValue> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut sequence = vec![start_state.clone()];
        let mut current_state = start_state;

        for _ in 0..steps {
            if let Some(targets) = self.transitions.get(&current_state) {
                let p: f64 = rng.r#gen(); // Random float [0.0, 1.0)
                let mut cumulative = 0.0;
                let mut next_state = &current_state;

                for (target_state, prob) in targets {
                    cumulative += prob;
                    if p <= cumulative {
                        next_state = target_state;
                        break;
                    }
                }

                current_state = next_state.clone();
                sequence.push(current_state.clone());
            } else {
                // Absorbing state (no outgoing transitions)
                break;
            }
        }

        sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_markov_chain_training_and_generation() {
        let heading = TupleType::new()
            .with_attribute("seq_id", ScalarType::Int)
            .with_attribute("step", ScalarType::Int)
            .with_attribute("weather", ScalarType::String);

        let mut log = Relation::new(RelationType::new(heading));

        // Sequence 1: Sunny -> Rain -> Sunny -> Sunny
        // Transitions:
        // Sunny -> Rain (1)
        // Rain -> Sunny (1)
        // Sunny -> Sunny (1)
        // Total from Sunny: 2 (50% Rain, 50% Sunny)
        // Total from Rain: 1 (100% Sunny)
        log.insert(tuple! { seq_id: 1i64, step: 1i64, weather: "Sunny" })
            .unwrap();
        log.insert(tuple! { seq_id: 1i64, step: 2i64, weather: "Rain" })
            .unwrap();
        log.insert(tuple! { seq_id: 1i64, step: 3i64, weather: "Sunny" })
            .unwrap();
        log.insert(tuple! { seq_id: 1i64, step: 4i64, weather: "Sunny" })
            .unwrap();

        let mut model = MarkovChain::new();
        model.train(&log, "seq_id", "step", "weather").unwrap();

        let sunny = ScalarValue::String("Sunny".to_string());
        let rain = ScalarValue::String("Rain".to_string());

        // Check probabilities
        let sunny_transitions = model.transitions.get(&sunny).unwrap();
        assert_eq!(sunny_transitions.len(), 2);

        // Since we sort by state value: "Rain" < "Sunny"
        assert_eq!(sunny_transitions[0].0, rain);
        assert_eq!(sunny_transitions[0].1, 0.5);

        assert_eq!(sunny_transitions[1].0, sunny);
        assert_eq!(sunny_transitions[1].1, 0.5);

        let rain_transitions = model.transitions.get(&rain).unwrap();
        assert_eq!(rain_transitions.len(), 1);
        assert_eq!(rain_transitions[0].0, sunny);
        assert_eq!(rain_transitions[0].1, 1.0);

        // Generate sequence
        // We use a fixed seed, which should generate a deterministic sequence.
        let seq = model.generate(rain.clone(), 3, 123);
        assert_eq!(seq.len(), 4);
        assert_eq!(seq[0], rain);
        assert_eq!(seq[1], sunny); // Rain -> Sunny is 100%
        // The rest depend on the RNG, but let's just ensure it generated the right length
    }
}
