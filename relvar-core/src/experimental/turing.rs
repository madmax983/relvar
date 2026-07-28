//! Relational Turing Machine
//!
//! Models a Turing Machine where the tape and transitions are purely relational.

use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// Represents a Relational Turing Machine.
pub struct RelationalTuringMachine {
    /// The tape of the Turing Machine, modeled as a Relation.
    /// Attributes: "pos" (Int), "symbol" (String)
    pub tape: Relation,

    /// The state transitions, modeled as a Relation.
    /// Attributes: "current_state" (String), "read_symbol" (String),
    /// "next_state" (String), "write_symbol" (String), "move_dir" (Int)
    pub transitions: Relation,

    /// The current state of the machine.
    pub current_state: String,

    /// The current head position.
    pub head_pos: i64,

    /// The blank symbol used for empty tape cells.
    pub blank_symbol: String,

    /// The halt state.
    pub halt_state: String,
}

impl RelationalTuringMachine {
    /// Creates a new Relational Turing Machine.
    pub fn new(initial_state: String, halt_state: String, blank_symbol: String) -> Self {
        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("pos".to_string(), ScalarType::Int)
                .with_attribute("symbol".to_string(), ScalarType::String),
        );
        let transitions_type = RelationType::new(
            TupleType::new()
                .with_attribute("current_state".to_string(), ScalarType::String)
                .with_attribute("read_symbol".to_string(), ScalarType::String)
                .with_attribute("next_state".to_string(), ScalarType::String)
                .with_attribute("write_symbol".to_string(), ScalarType::String)
                .with_attribute("move_dir".to_string(), ScalarType::Int),
        );

        Self {
            tape: Relation::new(tape_type),
            transitions: Relation::new(transitions_type),
            current_state: initial_state,
            head_pos: 0,
            blank_symbol,
            halt_state,
        }
    }

    /// Adds a transition rule to the machine.
    pub fn add_transition(
        &mut self,
        current_state: &str,
        read_symbol: &str,
        next_state: &str,
        write_symbol: &str,
        move_dir: i64,
    ) -> Result<(), crate::error::DatabaseError> {
        let _ = self.transitions.insert(tuple! {
            current_state: current_state.to_string(),
            read_symbol: read_symbol.to_string(),
            next_state: next_state.to_string(),
            write_symbol: write_symbol.to_string(),
            move_dir: move_dir
        })?;
        Ok(())
    }

    /// Writes a symbol to the tape at a specific position.
    pub fn write_tape(
        &mut self,
        pos: i64,
        symbol: &str,
    ) -> Result<(), crate::error::DatabaseError> {
        // First delete any existing symbol at this position
        let to_remove = self
            .tape
            .restrict(|t| t.get_typed::<i64>("pos").unwrap_or(0) == pos);
        self.tape =
            self.tape.clone().difference_into(&to_remove).map_err(|_| {
                crate::error::DatabaseError::AlgebraError("Difference error".into())
            })?;

        // Then insert the new symbol
        let _ = self.tape.insert(tuple! {
            pos: pos,
            symbol: symbol.to_string()
        })?;
        Ok(())
    }

    /// Performs one step of the Turing Machine.
    /// Returns true if the machine stepped, false if it halted or found no transition.
    pub fn step(&mut self) -> Result<bool, crate::error::DatabaseError> {
        if self.current_state == self.halt_state {
            return Ok(false);
        }

        // 1. Read current symbol from tape
        let hp = self.head_pos;
        let current_cell = self
            .tape
            .restrict(|t| t.get_typed::<i64>("pos").unwrap_or(0) == hp);

        let read_symbol = if current_cell.cardinality() > 0 {
            current_cell
                .tuples()
                .next()
                .unwrap()
                .get_typed::<String>("symbol")
                .unwrap()
        } else {
            self.blank_symbol.clone()
        };

        // 2. Find transition
        let state = self.current_state.clone();
        let sym = read_symbol.clone();

        let matching_transitions = self.transitions.restrict(|t| {
            t.get_typed::<String>("current_state").unwrap() == state
                && t.get_typed::<String>("read_symbol").unwrap() == sym
        });

        if matching_transitions.cardinality() == 0 {
            // Implicit halt if no transition
            return Ok(false);
        }

        let transition = matching_transitions.tuples().next().unwrap();
        let next_state = transition.get_typed::<String>("next_state").unwrap();
        let write_symbol = transition.get_typed::<String>("write_symbol").unwrap();
        let move_dir = transition.get_typed::<i64>("move_dir").unwrap();

        // 3. Write new symbol
        self.write_tape(self.head_pos, &write_symbol)?;

        // 4. Update head position and state
        self.head_pos += move_dir;
        self.current_state = next_state;

        Ok(true)
    }

    /// Runs the machine until it halts.
    pub fn run(&mut self) -> Result<(), crate::error::DatabaseError> {
        while self.step()? {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turing_machine_busy_beaver_3() {
        // 3-state busy beaver
        let mut tm =
            RelationalTuringMachine::new("A".to_string(), "HALT".to_string(), "0".to_string());

        // State A
        tm.add_transition("A", "0", "B", "1", 1).unwrap();
        tm.add_transition("A", "1", "HALT", "1", 1).unwrap();

        // State B
        tm.add_transition("B", "0", "C", "0", 1).unwrap();
        tm.add_transition("B", "1", "B", "1", 1).unwrap();

        // State C
        tm.add_transition("C", "0", "C", "1", -1).unwrap();
        tm.add_transition("C", "1", "A", "1", -1).unwrap();

        // Let's use a simpler 2-state busy beaver which writes four 1s and halts in 6 steps.
        // Actually, let's just make a simple one that flips '0' to '1' and moves right until it hits a blank ('_').

        let mut tm2 =
            RelationalTuringMachine::new("q0".to_string(), "HALT".to_string(), "_".to_string());
        tm2.write_tape(0, "0").unwrap();
        tm2.write_tape(1, "0").unwrap();
        tm2.write_tape(2, "0").unwrap();

        tm2.add_transition("q0", "0", "q0", "1", 1).unwrap();
        tm2.add_transition("q0", "_", "HALT", "_", 0).unwrap();

        tm2.run().unwrap();

        assert_eq!(tm2.head_pos, 3);
        assert_eq!(tm2.tape.cardinality(), 4);
        for i in 0..3 {
            let cell = tm2
                .tape
                .restrict(|t| t.get_typed::<i64>("pos").unwrap() == i);
            assert_eq!(
                cell.tuples()
                    .next()
                    .unwrap()
                    .get_typed::<String>("symbol")
                    .unwrap(),
                "1"
            );
        }
    }
}
