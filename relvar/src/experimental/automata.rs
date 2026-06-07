//! Relational Automata
//!
//! This module models Non-deterministic Finite Automata (NFA) using purely
//! relational algebra. States, transitions, and epsilon-closures are computed
//! using relational operations like `join`, `project`, `restrict`, and `tclose`.
//!
//! # Concepts
//!
//! - **States:** A relation containing `state` (String), `is_start` (Bool), and `is_accept` (Bool).
//! - **Transitions:** A relation containing `from_state` (String), `symbol` (String), and `to_state` (String).
//! - **Epsilon Transitions:** Represented by transitions where `symbol` is an empty string `""`.
//!
//! # Evaluation
//!
//! Given an input string, the NFA computes the active set of states for each character
//! iteratively by applying standard relational operators. The epsilon closure is computed
//! statically using `tclose` (transitive closure) to handle paths of arbitrary length.

use relvar_core::error::DatabaseError;
use relvar_core::values::Relation;

/// A Non-deterministic Finite Automaton (NFA) evaluated using relational algebra.
///
/// # Examples
///
/// ```text
/// // Example
/// ```
pub struct RelationalNfa {
    states: Relation,
    transitions: Relation,
}

impl RelationalNfa {
    /// Creates a new NFA.
    ///
    /// `states` must have schema `(state: String, is_start: Bool, is_accept: Bool)`
    /// `transitions` must have schema `(from_state: String, symbol: String, to_state: String)`
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new(states: Relation, transitions: Relation) -> Self {
        Self {
            states,
            transitions,
        }
    }

    /// Evaluates whether the NFA accepts the given string.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn accepts(&self, input: &str) -> Result<bool, DatabaseError> {
        let epsilon_closure = self.compute_epsilon_closure()?;
        let mut active_states = self.get_initial_active_states()?;
        active_states = Self::apply_epsilon_closure(&active_states, &epsilon_closure)?;

        let states = self.process_input_string(input, active_states, &epsilon_closure)?;
        if states.cardinality() > 0 {
            self.check_accepting_states(&states)
        } else {
            Ok(false)
        }
    }

    fn compute_epsilon_closure(&self) -> Result<Relation, DatabaseError> {
        let epsilon_transitions = self
            .transitions
            .clone()
            .restrict(|t| t.get_typed::<String>("symbol").unwrap() == "")
            .project(&["from_state", "to_state"]);

        let all_states = self.states.project(&["state"]);
        let from_states = all_states.rename(&[("state", "from_state")]);
        let to_states = all_states.rename(&[("state", "to_state")]);

        let cartesian = from_states.join(&to_states)?;
        let identity_transitions = cartesian.restrict(|t| {
            t.get_typed::<String>("from_state").unwrap()
                == t.get_typed::<String>("to_state").unwrap()
        });

        let mut epsilon_closure = identity_transitions;
        if epsilon_transitions.cardinality() > 0 {
            let tclosed = epsilon_transitions.tclose("from_state", "to_state")?;
            epsilon_closure = epsilon_closure
                .union(&tclosed)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        }
        Ok(epsilon_closure)
    }

    fn get_initial_active_states(&self) -> Result<Relation, DatabaseError> {
        Ok(self
            .states
            .clone()
            .restrict(|t| t.get_typed::<bool>("is_start").unwrap_or(false))
            .project(&["state"]))
    }

    fn apply_epsilon_closure(
        active: &Relation,
        e_close: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let renamed_active = active.rename(&[("state", "from_state")]);
        let joined = renamed_active.join(e_close)?;
        Ok(joined
            .project(&["to_state"])
            .rename(&[("to_state", "state")]))
    }

    fn process_input_string(
        &self,
        input: &str,
        mut active_states: Relation,
        epsilon_closure: &Relation,
    ) -> Result<Relation, DatabaseError> {
        for c in input.chars() {
            let symbol_str = c.to_string();

            let renamed_active = active_states.rename(&[("state", "from_state")]);
            let symbol_transitions = self
                .transitions
                .clone()
                .restrict(|t| t.get_typed::<String>("symbol").unwrap() == symbol_str);

            let next_states = renamed_active.join(&symbol_transitions)?;
            active_states = next_states
                .project(&["to_state"])
                .rename(&[("to_state", "state")]);

            active_states = Self::apply_epsilon_closure(&active_states, epsilon_closure)?;

            if active_states.cardinality() == 0 {
                return Ok(active_states);
            }
        }
        Ok(active_states)
    }

    fn check_accepting_states(&self, active_states: &Relation) -> Result<bool, DatabaseError> {
        let accept_states = self
            .states
            .clone()
            .restrict(|t| t.get_typed::<bool>("is_accept").unwrap_or(false))
            .project(&["state"]);

        let intersection = active_states.join(&accept_states)?;
        Ok(intersection.cardinality() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn build_nfa() -> RelationalNfa {
        let state_heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("is_start", ScalarType::Bool)
            .with_attribute("is_accept", ScalarType::Bool);
        let mut states = Relation::new(RelationType::new(state_heading));

        states
            .insert(tuple! { state: "q0", is_start: true, is_accept: false })
            .unwrap();
        states
            .insert(tuple! { state: "q1", is_start: false, is_accept: false })
            .unwrap();
        states
            .insert(tuple! { state: "q2", is_start: false, is_accept: true })
            .unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("from_state", ScalarType::String)
            .with_attribute("symbol", ScalarType::String)
            .with_attribute("to_state", ScalarType::String);
        let mut transitions = Relation::new(RelationType::new(trans_heading));

        // q0 --"a"--> q1
        transitions
            .insert(tuple! { from_state: "q0", symbol: "a", to_state: "q1" })
            .unwrap();
        // q1 --"b"--> q2
        transitions
            .insert(tuple! { from_state: "q1", symbol: "b", to_state: "q2" })
            .unwrap();
        // q1 --""--> q2 (epsilon)
        transitions
            .insert(tuple! { from_state: "q1", symbol: "", to_state: "q2" })
            .unwrap();
        // q2 --"c"--> q2 (loop)
        transitions
            .insert(tuple! { from_state: "q2", symbol: "c", to_state: "q2" })
            .unwrap();

        RelationalNfa::new(states, transitions)
    }

    #[test]
    fn test_nfa_ab() {
        let nfa = build_nfa();
        assert!(nfa.accepts("ab").unwrap());
    }

    #[test]
    fn test_nfa_a() {
        let nfa = build_nfa();
        assert!(nfa.accepts("a").unwrap()); // q0 -a-> q1 -e-> q2 (accept)
    }

    #[test]
    fn test_nfa_ac() {
        let nfa = build_nfa();
        assert!(nfa.accepts("ac").unwrap()); // q0 -a-> q1 -e-> q2 -c-> q2
    }

    #[test]
    fn test_nfa_abc() {
        let nfa = build_nfa();
        assert!(nfa.accepts("abc").unwrap()); // q0 -a-> q1 -b-> q2 -c-> q2
    }

    #[test]
    fn test_nfa_reject() {
        let nfa = build_nfa();
        assert!(!nfa.accepts("b").unwrap());
        assert!(!nfa.accepts("").unwrap());
        assert!(!nfa.accepts("abccx").unwrap());
    }

    #[test]
    fn test_nfa_star() {
        // a* NFA
        let state_heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("is_start", ScalarType::Bool)
            .with_attribute("is_accept", ScalarType::Bool);
        let mut states = Relation::new(RelationType::new(state_heading));

        states
            .insert(tuple! { state: "q0", is_start: true, is_accept: true })
            .unwrap();
        states
            .insert(tuple! { state: "q1", is_start: false, is_accept: false })
            .unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("from_state", ScalarType::String)
            .with_attribute("symbol", ScalarType::String)
            .with_attribute("to_state", ScalarType::String);
        let mut transitions = Relation::new(RelationType::new(trans_heading));

        // q0 --"a"--> q1
        transitions
            .insert(tuple! { from_state: "q0", symbol: "a", to_state: "q1" })
            .unwrap();
        // q1 --""--> q0 (epsilon loop)
        transitions
            .insert(tuple! { from_state: "q1", symbol: "", to_state: "q0" })
            .unwrap();

        let nfa = RelationalNfa::new(states, transitions);

        assert!(nfa.accepts("").unwrap());
        assert!(nfa.accepts("a").unwrap());
        assert!(nfa.accepts("aa").unwrap());
        assert!(nfa.accepts("aaa").unwrap());
        assert!(!nfa.accepts("b").unwrap());
        assert!(!nfa.accepts("ab").unwrap());
    }

    #[test]
    fn test_nfa_union() {
        // (ab)|(ac) NFA
        let state_heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("is_start", ScalarType::Bool)
            .with_attribute("is_accept", ScalarType::Bool);
        let mut states = Relation::new(RelationType::new(state_heading));

        states
            .insert(tuple! { state: "start", is_start: true, is_accept: false })
            .unwrap();
        states
            .insert(tuple! { state: "b1", is_start: false, is_accept: false })
            .unwrap();
        states
            .insert(tuple! { state: "b2", is_start: false, is_accept: true })
            .unwrap();
        states
            .insert(tuple! { state: "c1", is_start: false, is_accept: false })
            .unwrap();
        states
            .insert(tuple! { state: "c2", is_start: false, is_accept: true })
            .unwrap();
        states
            .insert(tuple! { state: "b1_a", is_start: false, is_accept: false })
            .unwrap();
        states
            .insert(tuple! { state: "c1_a", is_start: false, is_accept: false })
            .unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("from_state", ScalarType::String)
            .with_attribute("symbol", ScalarType::String)
            .with_attribute("to_state", ScalarType::String);
        let mut transitions = Relation::new(RelationType::new(trans_heading));

        // Branch 1: ab
        transitions
            .insert(tuple! { from_state: "start", symbol: "", to_state: "b1" })
            .unwrap();
        transitions
            .insert(tuple! { from_state: "b1", symbol: "a", to_state: "b1_a" })
            .unwrap();
        transitions
            .insert(tuple! { from_state: "b1_a", symbol: "b", to_state: "b2" })
            .unwrap();

        // Branch 2: ac
        transitions
            .insert(tuple! { from_state: "start", symbol: "", to_state: "c1" })
            .unwrap();
        transitions
            .insert(tuple! { from_state: "c1", symbol: "a", to_state: "c1_a" })
            .unwrap();
        transitions
            .insert(tuple! { from_state: "c1_a", symbol: "c", to_state: "c2" })
            .unwrap();

        let nfa = RelationalNfa::new(states, transitions);

        assert!(nfa.accepts("ab").unwrap());
        assert!(nfa.accepts("ac").unwrap());
        assert!(!nfa.accepts("a").unwrap());
        assert!(!nfa.accepts("abc").unwrap());
    }
}
