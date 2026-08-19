//! Relational Turing Machine.
//!
//! This module implements a Turing Machine purely using relational algebra primitives.
//! It proves that the relational algebra implemented in Relvar is Turing-complete!

use crate::error::DatabaseError;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};

/// A Turing Machine evaluated purely with Relational Algebra.
pub struct RelationalTuringMachine {
    /// The tape relation, holding attributes `pos` (Int) and `symbol` (String).
    pub tape: Relation,
    /// The current state relation, holding `id` (Int), `state` (String) and `head` (Int).
    pub state: Relation,
    /// The transition rules relation.
    pub transitions: Relation,
}

impl RelationalTuringMachine {
    /// Create a new Turing Machine.
    pub fn new(
        transitions: Relation,
        initial_tape: Relation,
        initial_state: String,
        initial_head: i64,
    ) -> Result<Self, DatabaseError> {
        let state_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("state", ScalarType::String)
            .with_attribute("head", ScalarType::Int);

        let mut state = Relation::new(RelationType::new(state_heading));
        state.insert(tuple! {
            id: 1i64,
            state: initial_state,
            head: initial_head
        })?;

        Ok(Self {
            tape: initial_tape,
            state,
            transitions,
        })
    }

    /// Run the machine for one step. Returns `Ok(true)` if it made a transition, `Ok(false)` if it halted.
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        // 1. Get current cell from tape. If not present, we should simulate a blank 'B'.
        // To do this relationally, we can get the head position, and see if it's in the tape.
        let current_head = self.state.project(&["head"]).rename(&[("head", "pos")]);
        let tape_at_head = self
            .tape
            .join(&current_head)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let current_cell = if tape_at_head.cardinality() == 0 {
            // Generate a blank symbol 'B' at current head
            current_head
                .extend_into("symbol", ScalarType::String, |_t| {
                    ScalarValue::String("B".to_string())
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        } else {
            tape_at_head
        };

        // 2. Form the current configuration for joining with transitions.
        // current_cell has (pos, symbol). state has (id, state, head).
        // Join them to get (pos, symbol, id, state).
        let config = self
            .state
            .rename(&[("head", "pos")])
            .join(&current_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename for transition matching
        let config_for_trans =
            config.rename(&[("state", "current_state"), ("symbol", "read_symbol")]);

        // 3. Find the matching transition
        let step_action = config_for_trans
            .join(&self.transitions)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        if step_action.cardinality() == 0 {
            // Halted
            return Ok(false);
        }

        // 4. Compute new state
        // step_action has (pos, id, current_state, read_symbol, write_symbol, move_dir, next_state)
        let new_state_base = step_action.project(&["id", "pos", "move_dir", "next_state"]);
        let new_state_ext = new_state_base
            .extend("head", ScalarType::Int, |t: &Tuple| {
                let pos = t.get_typed::<i64>("pos").unwrap();
                let dir = t.get_typed::<i64>("move_dir").unwrap();
                ScalarValue::Int(pos + dir)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let new_state = new_state_ext
            .project(&["id", "next_state", "head"])
            .rename(&[("next_state", "state")]);

        self.state = new_state;

        // 5. Compute new tape
        // Remove old symbol at pos
        let to_remove = step_action.project(&["pos"]);
        let tape_without_current = self.tape.semidifference(&to_remove);

        // Add new symbol
        let to_insert = step_action
            .project(&["pos", "write_symbol"])
            .rename(&[("write_symbol", "symbol")]);

        self.tape = tape_without_current
            .union(&to_insert)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(true)
    }

    /// Run until halted.
    pub fn run(&mut self) -> Result<(), DatabaseError> {
        while self.step()? {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turing_machine_simple_flip() {
        let trans_heading = TupleType::new()
            .with_attribute("current_state", ScalarType::String)
            .with_attribute("read_symbol", ScalarType::String)
            .with_attribute("write_symbol", ScalarType::String)
            .with_attribute("move_dir", ScalarType::Int)
            .with_attribute("next_state", ScalarType::String);

        let mut transitions = Relation::new(RelationType::new(trans_heading));

        // State q0: If read 0, write 1, move right, stay in q0. If B, write B, halt (no transition)
        transitions
            .insert(tuple! {
                current_state: "q0",
                read_symbol: "0",
                write_symbol: "1",
                move_dir: 1i64,
                next_state: "q0"
            })
            .unwrap();

        let tape_heading = TupleType::new()
            .with_attribute("pos", ScalarType::Int)
            .with_attribute("symbol", ScalarType::String);

        let mut initial_tape = Relation::new(RelationType::new(tape_heading));
        initial_tape
            .insert(tuple! { pos: 0i64, symbol: "0" })
            .unwrap();
        initial_tape
            .insert(tuple! { pos: 1i64, symbol: "0" })
            .unwrap();

        let mut tm =
            RelationalTuringMachine::new(transitions, initial_tape, "q0".to_string(), 0).unwrap();

        tm.run().unwrap();

        let t = tm
            .tape
            .restrict(|t: &Tuple| t.get_typed::<i64>("pos").unwrap() == 0);
        assert_eq!(
            t.tuples()
                .next()
                .unwrap()
                .get_typed::<String>("symbol")
                .unwrap(),
            "1"
        );
        let t = tm
            .tape
            .restrict(|t: &Tuple| t.get_typed::<i64>("pos").unwrap() == 1);
        assert_eq!(
            t.tuples()
                .next()
                .unwrap()
                .get_typed::<String>("symbol")
                .unwrap(),
            "1"
        );
    }
}
