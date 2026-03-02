//! Relational Finite State Machines (NFA/DFA).
//!
//! This module implements finite state machines using pure relational algebra.
//! Because relations are sets of tuples, this naturally models
//! Non-deterministic Finite Automata (NFAs). A DFA is just an NFA where
//! the current state relation always has a maximum cardinality of 1.
//!
//! State transitions are evaluated by performing a natural join between
//! the current states, the input symbol, and the transition table.

use relvar_core::error::DatabaseError;
use relvar_core::values::Relation;

/// A relational representation of a Non-deterministic Finite Automaton (NFA).
pub struct RelationalNFA {
    /// Transition table: `(state, symbol, next_state)`
    transitions: Relation,
    /// Current active states: `(state)`
    active_states: Relation,
    /// Accepting states: `(state)`
    accepting_states: Relation,
}

impl RelationalNFA {
    /// Creates a new Relational NFA.
    ///
    /// # Arguments
    ///
    /// * `transitions` - A relation with heading `(state, symbol, next_state)`
    /// * `initial_states` - A relation with heading `(state)`
    /// * `accepting_states` - A relation with heading `(state)`
    pub fn new(
        transitions: Relation,
        initial_states: Relation,
        accepting_states: Relation,
    ) -> Self {
        Self {
            transitions,
            active_states: initial_states,
            accepting_states,
        }
    }

    /// Processes a single input symbol represented as a relation.
    ///
    /// The `symbol` relation should typically contain a single tuple with
    /// heading `(symbol)`.
    pub fn step(&mut self, symbol: &Relation) -> Result<(), DatabaseError> {
        // 1. Join active states with the input symbol.
        // If active_states = {(state: "q0")} and symbol = {(symbol: "a")}
        // joined = {(state: "q0", symbol: "a")}
        // Note: active_states and symbol have disjoint headings, so join is Cartesian product.
        let current_config = self.active_states.join(symbol)?;

        // 2. Join with transition table to find next states.
        // transitions = {(state: "q0", symbol: "a", next_state: "q1"), ...}
        // matches = {(state: "q0", symbol: "a", next_state: "q1")}
        let matches = current_config.join(&self.transitions)?;

        // 3. Project to next_state and rename to state for the next iteration.
        let next_states_projected = matches.project(&["next_state"]);

        self.active_states = next_states_projected.rename(&[("next_state", "state")]);

        Ok(())
    }

    /// Checks if the NFA is currently in an accepting state.
    pub fn is_accepting(&self) -> Result<bool, DatabaseError> {
        let accepted = self
            .active_states
            .intersect(&self.accepting_states)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(!accepted.is_empty())
    }

    /// Returns the current active states.
    pub fn active_states(&self) -> &Relation {
        &self.active_states
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_nfa_recognition() {
        // Define NFA that accepts strings ending with "01"
        // States: q0, q1, q2
        // Initial: q0
        // Accepting: q2
        // Transitions:
        // (q0, "0") -> q0
        // (q0, "1") -> q0
        // (q0, "0") -> q1
        // (q1, "1") -> q2

        let state_type = ScalarType::String;
        let symbol_type = ScalarType::String;

        // Transitions
        let trans_heading = TupleType::new()
            .with_attribute("state", state_type.clone())
            .with_attribute("symbol", symbol_type.clone())
            .with_attribute("next_state", state_type.clone());
        let mut transitions = Relation::new(RelationType::new(trans_heading));
        transitions
            .insert(tuple! { state: "q0", symbol: "0", next_state: "q0" })
            .unwrap();
        transitions
            .insert(tuple! { state: "q0", symbol: "1", next_state: "q0" })
            .unwrap();
        transitions
            .insert(tuple! { state: "q0", symbol: "0", next_state: "q1" })
            .unwrap();
        transitions
            .insert(tuple! { state: "q1", symbol: "1", next_state: "q2" })
            .unwrap();

        // Initial States
        let state_heading = TupleType::new().with_attribute("state", state_type.clone());
        let mut initial_states = Relation::new(RelationType::new(state_heading.clone()));
        initial_states.insert(tuple! { state: "q0" }).unwrap();

        // Accepting States
        let mut accepting_states = Relation::new(RelationType::new(state_heading.clone()));
        accepting_states.insert(tuple! { state: "q2" }).unwrap();

        let mut nfa = RelationalNFA::new(
            transitions,
            initial_states.clone(),
            accepting_states.clone(),
        );

        // Process "001"
        let symbol_heading = TupleType::new().with_attribute("symbol", symbol_type.clone());

        // Step 1: "0"
        let mut s0 = Relation::new(RelationType::new(symbol_heading.clone()));
        s0.insert(tuple! { symbol: "0" }).unwrap();
        nfa.step(&s0).unwrap();
        assert!(!nfa.is_accepting().unwrap());

        // Step 2: "0"
        let mut s1 = Relation::new(RelationType::new(symbol_heading.clone()));
        s1.insert(tuple! { symbol: "0" }).unwrap();
        nfa.step(&s1).unwrap();
        assert!(!nfa.is_accepting().unwrap());

        // Step 3: "1"
        let mut s2 = Relation::new(RelationType::new(symbol_heading.clone()));
        s2.insert(tuple! { symbol: "1" }).unwrap();
        nfa.step(&s2).unwrap();
        assert!(nfa.is_accepting().unwrap());

        // Should NOT accept "000"
        let mut nfa2 = RelationalNFA::new(
            nfa.transitions.clone(),
            initial_states.clone(),
            accepting_states.clone(),
        );
        let mut s3 = Relation::new(RelationType::new(symbol_heading.clone()));
        s3.insert(tuple! { symbol: "0" }).unwrap();
        nfa2.step(&s3).unwrap();
        let mut s4 = Relation::new(RelationType::new(symbol_heading.clone()));
        s4.insert(tuple! { symbol: "0" }).unwrap();
        nfa2.step(&s4).unwrap();
        let mut s5 = Relation::new(RelationType::new(symbol_heading.clone()));
        s5.insert(tuple! { symbol: "0" }).unwrap();
        nfa2.step(&s5).unwrap();
        assert!(!nfa2.is_accepting().unwrap());
    }
}
