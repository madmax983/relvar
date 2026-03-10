//! Relational Finite State Automaton (FSA) Evaluation.
//!
//! This module implements a finite state machine purely using relational algebra.
//! Transitions, current states, and final states are all represented as relations.
//! Input strings are processed by iteratively joining the current active states
//! with the transition relation.
//!
//! # Concepts
//!
//! - **Transitions**: A relation with heading `{ state: String, symbol: String, next_state: String }`.
//! - **Active States**: A relation with heading `{ state: String }`.
//! - **Accept States**: A relation with heading `{ state: String }`.
//!
//! By evaluating FSA purely via set operations, we can compute regular language
//! intersections or process multiple simultaneous state paths (NFA) intrinsically
//! without backtracking, since the active states set naturally handles multiple
//! concurrent execution branches.

use relvar_core::error::DatabaseError;
use relvar_core::values::Relation;

/// A Finite State Automaton that executes via relational algebra.
pub struct RelationalFsa {
    /// The transition relation `{ state: String, symbol: String, next_state: String }`.
    pub transitions: Relation,
    /// The accept states relation `{ state: String }`.
    pub accept_states: Relation,
}

impl RelationalFsa {
    /// Creates a new RelationalFsa from the given transitions and accept states.
    ///
    /// Validates that `transitions` has the correct heading `(state, symbol, next_state)`
    /// and `accept_states` has the correct heading `(state)`.
    pub fn new(transitions: Relation, accept_states: Relation) -> Result<Self, DatabaseError> {
        let t_head = transitions.relation_type().heading();
        if !t_head.has_attribute("state")
            || !t_head.has_attribute("symbol")
            || !t_head.has_attribute("next_state")
        {
            return Err(DatabaseError::AlgebraError(
                "Transitions must have (state, symbol, next_state) attributes".into(),
            ));
        }

        let a_head = accept_states.relation_type().heading();
        if !a_head.has_attribute("state") {
            return Err(DatabaseError::AlgebraError(
                "Accept states must have (state) attribute".into(),
            ));
        }

        Ok(Self {
            transitions,
            accept_states,
        })
    }

    /// Evaluates whether an input sequence is accepted by the automaton, starting
    /// from a set of initial states.
    ///
    /// Returns `true` if any of the active states after processing all symbols
    /// intersect with the `accept_states` relation.
    pub fn evaluate(
        &self,
        initial_states: &Relation,
        sequence: &[&str],
    ) -> Result<bool, DatabaseError> {
        let mut active_states = initial_states.clone();

        for symbol in sequence {
            if active_states.cardinality() == 0 {
                // Short-circuit: no valid states remaining
                return Ok(false);
            }

            // 1. Filter transitions for the current symbol
            let current_symbol = symbol.to_string();
            let valid_transitions = self.transitions.restrict(|t| {
                if let Some(sym) = t.get_typed::<String>("symbol") {
                    sym == current_symbol
                } else {
                    false
                }
            });

            // 2. Join active states with valid transitions.
            // active_states: { state }
            // valid_transitions: { state, symbol, next_state }
            // joined: { state, symbol, next_state }
            let joined = active_states.join(&valid_transitions)?;

            // 3. Project out the new states.
            // projected: { next_state }
            let projected = joined.project(&["next_state"]);

            // 4. Rename next_state back to state to prepare for the next iteration.
            // renamed: { state }
            active_states = projected.rename(&[("next_state", "state")]);
        }

        // Finally, intersect the active states with the accept states.
        let accepted = active_states
            .intersect(&self.accept_states)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(accepted.cardinality() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn build_state_relation(states: &[&str]) -> Relation {
        let heading = TupleType::new().with_attribute("state", ScalarType::String);
        let mut rel = Relation::new(RelationType::new(heading.clone()));
        for state in states {
            let t = tuple! { state: state.to_string() };
            rel.insert(t).unwrap();
        }
        rel
    }

    fn build_transitions(trans: &[(&str, &str, &str)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("symbol", ScalarType::String)
            .with_attribute("next_state", ScalarType::String);
        let mut rel = Relation::new(RelationType::new(heading));
        for (state, symbol, next_state) in trans {
            let t = tuple! {
                state: state.to_string(),
                symbol: symbol.to_string(),
                next_state: next_state.to_string()
            };
            rel.insert(t).unwrap();
        }
        rel
    }

    #[test]
    fn test_fsa_matching_ab_plus() {
        // Regex: (ab)+
        // States: q0 (start), q1, q2 (accept)
        // Transitions:
        // q0 --a--> q1
        // q1 --b--> q2
        // q2 --a--> q1
        let transitions =
            build_transitions(&[("q0", "a", "q1"), ("q1", "b", "q2"), ("q2", "a", "q1")]);

        let accept_states = build_state_relation(&["q2"]);
        let start_states = build_state_relation(&["q0"]);

        let fsa = RelationalFsa::new(transitions, accept_states).unwrap();

        // Matches
        assert!(fsa.evaluate(&start_states, &["a", "b"]).unwrap());
        assert!(fsa.evaluate(&start_states, &["a", "b", "a", "b"]).unwrap());
        assert!(
            fsa.evaluate(&start_states, &["a", "b", "a", "b", "a", "b"])
                .unwrap()
        );

        // Doesn't match
        assert!(!fsa.evaluate(&start_states, &[]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["a"]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["a", "a"]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["a", "b", "a"]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["b"]).unwrap());
    }

    #[test]
    fn test_nfa_multiple_paths() {
        // NFA that matches strings ending in "01"
        // States: q0, q1, q2 (accept)
        // q0 --0--> q0, q1
        // q0 --1--> q0
        // q1 --1--> q2
        let transitions = build_transitions(&[
            ("q0", "0", "q0"),
            ("q0", "0", "q1"),
            ("q0", "1", "q0"),
            ("q1", "1", "q2"),
        ]);

        let accept_states = build_state_relation(&["q2"]);
        let start_states = build_state_relation(&["q0"]);

        let fsa = RelationalFsa::new(transitions, accept_states).unwrap();

        // Matches
        assert!(fsa.evaluate(&start_states, &["0", "1"]).unwrap());
        assert!(fsa.evaluate(&start_states, &["1", "0", "1"]).unwrap());
        assert!(fsa.evaluate(&start_states, &["0", "0", "0", "1"]).unwrap());

        // Doesn't match
        assert!(!fsa.evaluate(&start_states, &["0"]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["1"]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["0", "1", "0"]).unwrap());
        assert!(!fsa.evaluate(&start_states, &["1", "0", "0"]).unwrap());
    }
}
