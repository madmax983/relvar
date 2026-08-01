//! Relational Turing Machine
//!
//! Models a Turing Machine purely using relational algebra.
//! The tape, state, and transition function are all represented as relations.

use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, Tuple};
use crate::{DatabaseError, tuple};

/// A Relational Turing Machine.
pub struct TuringMachine {
    /// Tape relation with attributes: `position` (Int), `symbol` (String).
    pub tape: Relation,
    /// State relation with attributes: `current_state` (String), `head_position` (Int).
    pub state: Relation,
    /// Transitions relation with attributes: `current_state` (String), `read_symbol` (String),
    /// `write_symbol` (String), `move_direction` (Int), `next_state` (String).
    pub transitions: Relation,
}

impl TuringMachine {
    /// Creates a new Turing Machine.
    pub fn new(transitions: Relation, initial_state: String, initial_tape: Relation) -> Self {
        let state_type = RelationType::new(
            TupleType::new()
                .with_attribute("current_state", ScalarType::String)
                .with_attribute("head_position", ScalarType::Int),
        );
        let mut state = Relation::new(state_type);
        let _ = state.insert(tuple! { current_state: initial_state, head_position: 0i64 });

        Self {
            tape: initial_tape,
            state,
            transitions,
        }
    }

    /// Performs one step of the Turing Machine.
    pub fn step(&mut self, blank_symbol: &str) -> Result<bool, DatabaseError> {
        let head = self.state.project(&["head_position"]);
        let tape_head_pos = self
            .tape
            .clone()
            .rename_into(&[("position", "head_position")]);
        let tape_at_head = tape_head_pos
            .clone()
            .join(&head)
            .map_err(|_| DatabaseError::AlgebraError("join".into()))?;

        // If the tape doesn't have the cell, add a blank.
        if tape_at_head.cardinality() == 0 {
            let _ = self.tape.insert(tuple! {
                position: self.state.tuples().next().unwrap().get_typed::<i64>("head_position").unwrap(),
                symbol: blank_symbol.to_string()
            });
        }

        let tape_head_pos = self
            .tape
            .clone()
            .rename_into(&[("position", "head_position")]);

        // current_config has: current_state, head_position, symbol
        let current_config = self
            .state
            .clone()
            .join(&tape_head_pos)
            .map_err(|_| DatabaseError::AlgebraError("join".into()))?;

        let current_config_renamed = current_config.rename_into(&[("symbol", "read_symbol")]);
        let active_transition = current_config_renamed
            .join(&self.transitions)
            .map_err(|_| DatabaseError::AlgebraError("join".into()))?;

        if active_transition.cardinality() == 0 {
            // Halt
            return Ok(false);
        }

        // We can do it purely relationally for state update!
        // active_transition has: current_state, head_position, read_symbol, write_symbol, move_direction, next_state

        // Next State
        let next_state_proto =
            active_transition.project(&["next_state", "head_position", "move_direction"]);
        let mut next_state = next_state_proto
            .extend("new_head_position", ScalarType::Int, |t: &Tuple| {
                let pos = t.get_typed::<i64>("head_position").unwrap();
                let mov = t.get_typed::<i64>("move_direction").unwrap();
                crate::values::ScalarValue::Int(pos + mov)
            })
            .map_err(|_| DatabaseError::AlgebraError("extend".into()))?;
        next_state = next_state.project(&["next_state", "new_head_position"]);
        next_state = next_state.rename_into(&[
            ("next_state", "current_state"),
            ("new_head_position", "head_position"),
        ]);

        // Tape update
        // We need to overwrite the symbol at head_position with write_symbol
        let head_only = self.state.project(&["head_position"]);
        let head_only_pos = head_only.rename_into(&[("head_position", "position")]);
        let head_join = self
            .tape
            .clone()
            .join(&head_only_pos)
            .map_err(|_| DatabaseError::AlgebraError("join".into()))?;
        let tape_without_head = self
            .tape
            .clone()
            .difference_into(&head_join)
            .map_err(|_| DatabaseError::AlgebraError("difference".into()))?;

        let new_cell = active_transition
            .project(&["head_position", "write_symbol"])
            .rename_into(&[("head_position", "position"), ("write_symbol", "symbol")]);

        self.tape = tape_without_head
            .union_into(&new_cell)
            .map_err(|_| DatabaseError::AlgebraError("union".into()))?;
        self.state = next_state;

        Ok(true)
    }

    /// Runs the Turing machine until it halts.
    pub fn run_until_halt(&mut self, blank_symbol: &str) -> Result<(), DatabaseError> {
        while self.step(blank_symbol)? {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turing_machine_invert_bits() {
        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("current_state", ScalarType::String)
                .with_attribute("read_symbol", ScalarType::String)
                .with_attribute("write_symbol", ScalarType::String)
                .with_attribute("move_direction", ScalarType::Int)
                .with_attribute("next_state", ScalarType::String),
        );
        let mut transitions = Relation::new(trans_type);
        // State q0: read 0 -> write 1, move R, state q0
        let _ = transitions.insert(tuple! { current_state: "q0", read_symbol: "0", write_symbol: "1", move_direction: 1i64, next_state: "q0" });
        // State q0: read 1 -> write 0, move R, state q0
        let _ = transitions.insert(tuple! { current_state: "q0", read_symbol: "1", write_symbol: "0", move_direction: 1i64, next_state: "q0" });
        // State q0: read blank -> write blank, move L, state halt
        let _ = transitions.insert(tuple! { current_state: "q0", read_symbol: "B", write_symbol: "B", move_direction: -1i64, next_state: "halt" });

        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("position", ScalarType::Int)
                .with_attribute("symbol", ScalarType::String),
        );
        let mut tape = Relation::new(tape_type);
        let _ = tape.insert(tuple! { position: 0i64, symbol: "0" });
        let _ = tape.insert(tuple! { position: 1i64, symbol: "1" });
        let _ = tape.insert(tuple! { position: 2i64, symbol: "0" });

        let mut tm = TuringMachine::new(transitions, "q0".to_string(), tape);
        tm.run_until_halt("B").unwrap();

        // Final tape should be 1, 0, 1 at positions 0, 1, 2
        let tm_tape = tm
            .tape
            .clone()
            .restrict(|t| t.get_typed::<i64>("position").unwrap() == 0);
        let t0 = tm_tape.tuples().next().unwrap();
        assert_eq!(t0.get_typed::<String>("symbol").unwrap(), "1");
        let tm_tape1 = tm
            .tape
            .clone()
            .restrict(|t| t.get_typed::<i64>("position").unwrap() == 1);
        let t1 = tm_tape1.tuples().next().unwrap();
        assert_eq!(t1.get_typed::<String>("symbol").unwrap(), "0");
        let tm_tape2 = tm
            .tape
            .clone()
            .restrict(|t| t.get_typed::<i64>("position").unwrap() == 2);
        let t2 = tm_tape2.tuples().next().unwrap();
        assert_eq!(t2.get_typed::<String>("symbol").unwrap(), "1");
    }
}
