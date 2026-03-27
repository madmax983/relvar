//! Relational Turing Machine
//!
//! This module demonstrates how a Turing Machine can be implemented using purely
//! relational algebra operations. The machine's state, tape, and transition rules
//! are all modeled as relations.
//!
//! # Concept
//!
//! - `tape`: A relation representing the tape. Schema: `(pos: Int, symbol: String)`
//! - `head`: A relation representing the current state and position. Schema: `(state: String, pos: Int)`
//! - `transitions`: A relation representing the transition function. Schema:
//!   `(current_state: String, read_symbol: String, new_state: String, write_symbol: String, move_dir: Int)`
//!
//! A single step of the Turing Machine is computed by joining the head, tape, and transition relations,
//! updating the tape using set difference and union, and computing the new head position using extend.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A relational Turing Machine.
pub struct TuringMachine {
    /// The tape of the Turing machine. Schema: `(pos: Int, symbol: String)`
    pub tape: Relation,
    /// The head of the Turing machine. Schema: `(state: String, pos: Int)`
    pub head: Relation,
    /// The transition function. Schema: `(current_state: String, read_symbol: String, new_state: String, write_symbol: String, move_dir: Int)`
    pub transitions: Relation,
    /// The blank symbol used on the tape.
    pub blank_symbol: String,
    /// The halting states.
    pub halt_states: std::collections::HashSet<String>,
}

impl TuringMachine {
    /// Creates a new TuringMachine.
    ///
    /// # Arguments
    ///
    /// * `tape` - A relation with heading `(pos: Int, symbol: String)`.
    /// * `head` - A relation with heading `(state: String, pos: Int)`.
    /// * `transitions` - A relation with heading `(current_state: String, read_symbol: String, new_state: String, write_symbol: String, move_dir: Int)`.
    /// * `blank_symbol` - The string representing a blank cell on the tape.
    /// * `halt_states` - A set of state names that cause the machine to halt.
    pub fn new(
        tape: Relation,
        head: Relation,
        transitions: Relation,
        blank_symbol: &str,
        halt_states: std::collections::HashSet<String>,
    ) -> Self {
        Self {
            tape,
            head,
            transitions,
            blank_symbol: blank_symbol.to_string(),
            halt_states,
        }
    }

    /// Executes a single step of the Turing Machine.
    ///
    /// Returns `true` if a transition was applied, `false` if the machine halted (no valid transition).
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        // 1. Read the current symbol under the head.
        // We do this by joining `head` and `tape` on `pos`.
        let current_cell = self.head.join(&self.tape)?;

        // If the tape has no symbol at the current pos, we need to treat it as a blank symbol.
        let head_pos = self.head.project(&["pos"]);
        let tape_pos = self.tape.project(&["pos"]);

        let missing_pos = head_pos
            .difference(&tape_pos)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let blank_symbol = self.blank_symbol.clone();
        let blank_cell = self
            .head
            .join(&missing_pos)?
            .extend("symbol", ScalarType::String, move |_| {
                ScalarValue::String(blank_symbol.clone())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Union the read cell with the blank cell (only one will have a tuple).
        let read_state = current_cell
            .union(&blank_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // `read_state` has schema: (state: String, pos: Int, symbol: String)
        // We rename to match `transitions` schema for joining.
        let rename_map = vec![("state", "current_state"), ("symbol", "read_symbol")];
        let state_for_join = read_state.rename(&rename_map);

        // 2. Find the applicable transition.
        let applied_transition = state_for_join.join(&self.transitions)?;

        // If no transition applies, we halt.
        if applied_transition.is_empty() {
            return Ok(false);
        }

        // 3. Update the tape.
        // The cell to update is at `pos`.
        // The new tape cell is `(pos, write_symbol)` -> rename write_symbol to symbol.
        let new_tape_cell = applied_transition
            .project(&["pos", "write_symbol"])
            .rename(&[("write_symbol", "symbol")]);

        // Remove the old cell at `pos` from the tape and union the new cell.
        let pos_to_replace = applied_transition.project(&["pos"]);
        let old_tape_cell = self.tape.join(&pos_to_replace)?;

        let tape_without_pos = self
            .tape
            .difference(&old_tape_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.tape = tape_without_pos
            .union(&new_tape_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Update the head.
        // new_state, new_pos = pos + move_dir
        let new_head = applied_transition
            .extend("new_pos", ScalarType::Int, |t| {
                let p = t.get_typed::<i64>("pos").unwrap();
                let m = t.get_typed::<i64>("move_dir").unwrap();
                ScalarValue::Int(p + m)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["new_state", "new_pos"])
            .rename(&[("new_state", "state"), ("new_pos", "pos")]);

        self.head = new_head;

        Ok(true)
    }

    /// Runs the Turing Machine until it halts or reaches the maximum number of steps.
    ///
    /// Returns the number of steps executed.
    pub fn run(&mut self, max_steps: usize) -> Result<usize, DatabaseError> {
        let mut steps = 0;

        while steps < max_steps {
            // Check if we are in a halting state.
            let mut is_halt = false;
            for t in self.head.tuples() {
                if let Some(ScalarValue::String(state)) = t.get("state") {
                    if self.halt_states.contains(state) {
                        is_halt = true;
                    }
                }
            }
            if is_halt {
                break;
            }

            let advanced = self.step()?;
            if !advanced {
                break;
            }
            steps += 1;
        }

        Ok(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_bit_flipper() {
        // A simple Turing machine that moves right, flipping 0s to 1s and 1s to 0s,
        // halting when it hits a blank.

        let tape_heading = TupleType::new()
            .with_attribute("pos", ScalarType::Int)
            .with_attribute("symbol", ScalarType::String);
        let mut tape = Relation::new(RelationType::new(tape_heading));

        // Initial tape: 010
        tape.insert(tuple! { pos: 0i64, symbol: "0" }).unwrap();
        tape.insert(tuple! { pos: 1i64, symbol: "1" }).unwrap();
        tape.insert(tuple! { pos: 2i64, symbol: "0" }).unwrap();

        let head_heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("pos", ScalarType::Int);
        let mut head = Relation::new(RelationType::new(head_heading));
        head.insert(tuple! { state: "q0", pos: 0i64 }).unwrap();

        let transitions_heading = TupleType::new()
            .with_attribute("current_state", ScalarType::String)
            .with_attribute("read_symbol", ScalarType::String)
            .with_attribute("new_state", ScalarType::String)
            .with_attribute("write_symbol", ScalarType::String)
            .with_attribute("move_dir", ScalarType::Int);
        let mut transitions = Relation::new(RelationType::new(transitions_heading));

        // Flip 0 to 1, move right, stay in q0
        transitions
            .insert(tuple! {
                current_state: "q0", read_symbol: "0",
                new_state: "q0", write_symbol: "1", move_dir: 1i64
            })
            .unwrap();

        // Flip 1 to 0, move right, stay in q0
        transitions
            .insert(tuple! {
                current_state: "q0", read_symbol: "1",
                new_state: "q0", write_symbol: "0", move_dir: 1i64
            })
            .unwrap();

        // Hit blank space, go to halt state (qH), stay in place
        transitions
            .insert(tuple! {
                current_state: "q0", read_symbol: " ",
                new_state: "qH", write_symbol: " ", move_dir: 0i64
            })
            .unwrap();

        let mut halt_states = std::collections::HashSet::new();
        halt_states.insert("qH".to_string());

        let mut tm = TuringMachine::new(tape, head, transitions, " ", halt_states);

        let steps = tm.run(10).unwrap();
        assert_eq!(steps, 4); // Flips pos 0, 1, 2, then at pos 3 it hits blank and halts.

        // Verify head is at pos 3 and state is qH
        let head_tuple = tm.head.tuples().next().unwrap();
        assert_eq!(head_tuple.get_typed::<String>("state").unwrap(), "qH");
        assert_eq!(head_tuple.get_typed::<i64>("pos").unwrap(), 3);

        // Verify tape content: 101
        let get_symbol = |pos| {
            tm.tape
                .tuples()
                .find(|t| t.get_typed::<i64>("pos").unwrap() == pos)
                .map(|t| t.get_typed::<String>("symbol").unwrap())
                .unwrap_or(" ".to_string())
        };

        assert_eq!(get_symbol(0), "1");
        assert_eq!(get_symbol(1), "0");
        assert_eq!(get_symbol(2), "1");
        // Blank that was written at the end
        assert_eq!(get_symbol(3), " ");
    }
}
