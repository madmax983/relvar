use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

/// A Relational Turing Machine Simulator.
///
/// Evaluates Turing Machine transitions using pure relational algebra.
pub struct TuringMachine {
    /// The tape relation
    pub tape: Relation,
    /// The head relation
    pub head: Relation,
    /// The transitions relation
    pub transitions: Relation,
    /// The blank symbol
    pub blank_symbol: String,
}

impl TuringMachine {
    /// Create a new Turing Machine.
    pub fn new(blank_symbol: &str) -> Self {
        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("position", ScalarType::Int)
                .with_attribute("symbol", ScalarType::String),
        );

        let head_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("position", ScalarType::Int),
        );

        let transitions_type = RelationType::new(
            TupleType::new()
                .with_attribute("current_state", ScalarType::String)
                .with_attribute("read_symbol", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("write_symbol", ScalarType::String)
                .with_attribute("move_dir", ScalarType::Int),
        );

        Self {
            tape: Relation::new(tape_type),
            head: Relation::new(head_type),
            transitions: Relation::new(transitions_type),
            blank_symbol: blank_symbol.to_string(),
        }
    }

    /// Steps the Turing machine using purely relational algebra.
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        // Ensure tape has a blank at the head's position if it's currently unmapped
        let head_positions = self.head.project(&["position"]);
        let tape_positions = self.tape.project(&["position"]);

        let missing_positions = head_positions
            .difference(&tape_positions)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let blank = self.blank_symbol.clone();
        let new_blanks = missing_positions
            .extend("symbol", ScalarType::String, move |_| {
                ScalarValue::String(blank.clone())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.tape = self
            .tape
            .union(&new_blanks)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 1. Join Head and Tape to find current state and read symbol
        let head_state = self.head.join(&self.tape)?;

        // head_state has: state, position, symbol
        // Rename to match transitions
        let head_state_renamed =
            head_state.rename(&[("state", "current_state"), ("symbol", "read_symbol")]);

        // 2. Find matching transition
        let matching_transition = head_state_renamed.join(&self.transitions)?;

        if matching_transition.is_empty() {
            // Halt: No transition found
            return Ok(false);
        }

        // 3. Compute new head
        let new_head_extended = matching_transition
            .extend("new_position", ScalarType::Int, |t| {
                let pos = t.get_typed::<i64>("position").unwrap();
                let move_dir = t.get_typed::<i64>("move_dir").unwrap();
                ScalarValue::Int(pos + move_dir)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let new_head_proj = new_head_extended.project(&["next_state", "new_position"]);

        self.head = new_head_proj.rename(&[("next_state", "state"), ("new_position", "position")]);

        // 4. Compute new tape
        let tape_update_raw = matching_transition.project(&["position", "write_symbol"]);
        let tape_update = tape_update_raw.rename(&[("write_symbol", "symbol")]);

        let update_positions = tape_update.project(&["position"]);

        // Find unchanged tape
        // To do this relationally, we can natural join tape with the positions we are NOT updating.
        // Or Tape MINUS (Tape JOIN update_positions)
        let old_tape_to_remove = self.tape.join(&update_positions)?;
        let unchanged_tape = self
            .tape
            .difference(&old_tape_to_remove)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.tape = unchanged_tape
            .union(&tape_update)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_turing_machine_invert_bits() {
        let mut tm = TuringMachine::new("_");

        // Tape: [1, 0, 1]
        tm.tape
            .insert(tuple! { position: 0i64, symbol: "1" })
            .unwrap();
        tm.tape
            .insert(tuple! { position: 1i64, symbol: "0" })
            .unwrap();
        tm.tape
            .insert(tuple! { position: 2i64, symbol: "1" })
            .unwrap();

        // Head: starts at 0, state "q0"
        tm.head
            .insert(tuple! { state: "q0", position: 0i64 })
            .unwrap();

        // Transitions:
        // q0, 1 -> q0, 0, R(+1)
        // q0, 0 -> q0, 1, R(+1)
        // q0, _ -> halt (no transition)
        tm.transitions
            .insert(tuple! {
                current_state: "q0",
                read_symbol: "1",
                next_state: "q0",
                write_symbol: "0",
                move_dir: 1i64
            })
            .unwrap();
        tm.transitions
            .insert(tuple! {
                current_state: "q0",
                read_symbol: "0",
                next_state: "q0",
                write_symbol: "1",
                move_dir: 1i64
            })
            .unwrap();

        // Run
        let mut steps = 0;
        while tm.step().unwrap() {
            steps += 1;
        }

        assert_eq!(steps, 3);

        // Check head is at position 3
        let final_head = tm.head.tuples().next().unwrap();
        assert_eq!(final_head.get_typed::<i64>("position").unwrap(), 3);
        assert_eq!(final_head.get_typed::<String>("state").unwrap(), "q0");

        // Check tape is inverted
        let inverted_ones = tm
            .tape
            .restrict(|t| t.get_typed::<String>("symbol").unwrap() == "1");

        let ones_positions: Vec<i64> = inverted_ones
            .tuples()
            .map(|t| t.get_typed::<i64>("position").unwrap())
            .collect();

        assert_eq!(ones_positions, vec![1]); // Only position 1 should be '1'
    }
}
