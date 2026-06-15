//! Relational NFA (Non-deterministic Finite Automaton) Evaluator.
//!
//! Evaluates an NFA over a sequence of inputs using pure relational algebra.
//! States, transitions, and active states are modeled as relations.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Computes the epsilon-closure of a set of active states.
///
/// `active_states` has attribute `state` (Int).
/// `epsilon_transitions` has attributes `from_state` (Int), `to_state` (Int).
pub fn epsilon_closure(
    active_states: &Relation,
    epsilon_transitions: &Relation,
) -> Result<Relation, DatabaseError> {
    if epsilon_transitions.cardinality() == 0 {
        return Ok(active_states.clone());
    }

    // Compute the full transitive closure of epsilon transitions
    let closure = epsilon_transitions.tclose("from_state", "to_state")?;

    // Find all states reachable from the current active states
    let renamed_active = active_states.rename(&[("state", "from_state")]);

    // Join with closure
    let reachable = renamed_active.join(&closure)?;

    // Project out "from_state" and rename "to_state" back to "state"
    let new_states = reachable
        .project(&["to_state"])
        .rename(&[("to_state", "state")]);

    // The epsilon closure is the union of the original states and the reachable states
    active_states
        .union(&new_states)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

/// Evaluates one step of the NFA given an input token.
///
/// `active_states` has attribute `state` (Int).
/// `transitions` has attributes `from_state` (Int), `token` (String), `to_state` (Int).
/// `epsilon_transitions` has attributes `from_state` (Int), `to_state` (Int).
/// `input_token` is the token to consume.
pub fn nfa_step(
    active_states: &Relation,
    transitions: &Relation,
    epsilon_transitions: &Relation,
    input_token: &str,
) -> Result<Relation, DatabaseError> {
    // 1. Epsilon closure of current active states
    let closure_states = epsilon_closure(active_states, epsilon_transitions)?;

    // 2. Consume input token
    let renamed_active = closure_states.rename(&[("state", "from_state")]);

    // Restrict the transitions relation
    let target_token = input_token.to_string();
    let token_transitions = transitions
        .restrict(move |t| t.get_typed::<String>("token").unwrap_or_default() == target_token);

    // Join active states with matching transitions
    let next_states_raw = renamed_active.join(&token_transitions)?;

    // Project and rename to get new active states
    let next_states = next_states_raw
        .project(&["to_state"])
        .rename(&[("to_state", "state")]);

    // 3. Epsilon closure of the new states
    epsilon_closure(&next_states, epsilon_transitions)
}

/// Evaluates a full sequence of tokens against the NFA.
pub fn evaluate_nfa(
    start_states: Relation,
    transitions: Relation,
    epsilon_transitions: Relation,
    tokens: &[&str],
) -> Result<Relation, DatabaseError> {
    let mut current_states = epsilon_closure(&start_states, &epsilon_transitions)?;
    for token in tokens {
        current_states = nfa_step(&current_states, &transitions, &epsilon_transitions, token)?;
    }
    Ok(current_states)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_nfa_evaluation() {
        // NFA for regex: (a|b)*abb
        let state_type =
            RelationType::new(TupleType::new().with_attribute("state", ScalarType::Int));
        let mut start_states = Relation::new(state_type.clone());
        start_states.insert(tuple! { state: 0i64 }).unwrap();

        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_state", ScalarType::Int)
                .with_attribute("token", ScalarType::String)
                .with_attribute("to_state", ScalarType::Int),
        );
        let mut transitions = Relation::new(trans_type);
        transitions
            .insert(tuple! { from_state: 0i64, token: "a", to_state: 0i64 })
            .unwrap();
        transitions
            .insert(tuple! { from_state: 0i64, token: "b", to_state: 0i64 })
            .unwrap();
        transitions
            .insert(tuple! { from_state: 0i64, token: "a", to_state: 1i64 })
            .unwrap();
        transitions
            .insert(tuple! { from_state: 1i64, token: "b", to_state: 2i64 })
            .unwrap();
        transitions
            .insert(tuple! { from_state: 2i64, token: "b", to_state: 3i64 })
            .unwrap();

        let eps_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_state", ScalarType::Int)
                .with_attribute("to_state", ScalarType::Int),
        );
        let epsilon_transitions = Relation::new(eps_type);

        let tokens = vec!["a", "b", "a", "b", "b"];
        let final_states =
            evaluate_nfa(start_states, transitions, epsilon_transitions, &tokens).unwrap();

        assert!(final_states.contains(&tuple! { state: 3i64 }));
    }

    #[test]
    fn test_nfa_with_epsilon() {
        // NFA with epsilon transitions
        // 0 -(eps)-> 1 -(a)-> 2 -(eps)-> 3
        let state_type =
            RelationType::new(TupleType::new().with_attribute("state", ScalarType::Int));
        let mut start_states = Relation::new(state_type.clone());
        start_states.insert(tuple! { state: 0i64 }).unwrap();

        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_state", ScalarType::Int)
                .with_attribute("token", ScalarType::String)
                .with_attribute("to_state", ScalarType::Int),
        );
        let mut transitions = Relation::new(trans_type);
        transitions
            .insert(tuple! { from_state: 1i64, token: "a", to_state: 2i64 })
            .unwrap();

        let eps_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_state", ScalarType::Int)
                .with_attribute("to_state", ScalarType::Int),
        );
        let mut epsilon_transitions = Relation::new(eps_type);
        epsilon_transitions
            .insert(tuple! { from_state: 0i64, to_state: 1i64 })
            .unwrap();
        epsilon_transitions
            .insert(tuple! { from_state: 2i64, to_state: 3i64 })
            .unwrap();

        let tokens = vec!["a"];
        let final_states =
            evaluate_nfa(start_states, transitions, epsilon_transitions, &tokens).unwrap();

        assert!(final_states.contains(&tuple! { state: 3i64 }));
    }
}
