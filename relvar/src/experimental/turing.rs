//! Relational Turing Machine
//!
//! This module demonstrates that Relational Algebra is Turing Complete
//! by implementing a Turing Machine using pure relational operations.
//!
//! # Concept
//!
//! A Turing machine requires:
//! - An infinite tape of symbols
//! - A head that reads/writes the tape and moves left or right
//! - A state machine defining transitions
//!
//! In a relational context:
//! - **Tape**: A relation `(position: Int, symbol: String)`
//! - **Head**: A relation `(position: Int, state: String)` with cardinality 1
//! - **Transitions**: A relation `(current_state: String, read_symbol: String, next_state: String, write_symbol: String, move_dir: Int)`
//!
//! # How it works
//!
//! 1. Join `Head`, `Tape`, and `Transitions` to find the active transition.
//! 2. Calculate the new `Head` by projecting the new state and calculating the new position.
//! 3. Calculate the new `Tape` by updating the written symbol at the current position.
//! 4. Repeat until no transition is found (halt).

use relvar_core::{
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};
use std::collections::HashMap;

/// A Turing Machine implemented via Relational Algebra.
pub struct TuringMachine {
    /// The tape storing symbols. Schema: (pos: Int, symbol: String)
    pub tape: Relation,
    /// The read/write head. Schema: (pos: Int, state: String)
    pub head: Relation,
    /// The transition rules. Schema: (current_state: String, read_symbol: String, next_state: String, write_symbol: String, move_dir: Int)
    pub transitions: Relation,
    /// The blank symbol used for empty tape cells
    pub blank_symbol: String,
}

impl TuringMachine {
    /// Creates a new Turing Machine.
    pub fn new(tape: Relation, head: Relation, transitions: Relation, blank_symbol: &str) -> Self {
        Self {
            tape,
            head,
            transitions,
            blank_symbol: blank_symbol.to_string(),
        }
    }

    /// Helper to create a tape relation from a string
    pub fn create_tape(initial_data: &str) -> Result<Relation, DatabaseError> {
        let heading = TupleType::new()
            .with_attribute("pos".to_string(), ScalarType::Int)
            .with_attribute("symbol".to_string(), ScalarType::String);

        let mut tape = Relation::new(RelationType::new(heading.clone()));

        for (i, ch) in initial_data.chars().enumerate() {
            let mut vals = HashMap::new();
            vals.insert("pos".to_string(), ScalarValue::Int(i as i64));
            vals.insert("symbol".to_string(), ScalarValue::String(ch.to_string()));
            tape.insert(Tuple::new(heading.clone(), vals).unwrap())?;
        }

        Ok(tape)
    }

    /// Helper to create an initial head relation
    pub fn create_head(initial_state: &str, initial_pos: i64) -> Result<Relation, DatabaseError> {
        let heading = TupleType::new()
            .with_attribute("pos".to_string(), ScalarType::Int)
            .with_attribute("state".to_string(), ScalarType::String);

        let mut head = Relation::new(RelationType::new(heading.clone()));
        let mut vals = HashMap::new();
        vals.insert("pos".to_string(), ScalarValue::Int(initial_pos));
        vals.insert(
            "state".to_string(),
            ScalarValue::String(initial_state.to_string()),
        );
        head.insert(Tuple::new(heading, vals).unwrap())?;
        Ok(head)
    }

    /// Executes one step of the Turing machine.
    /// Returns true if a step was taken, false if it halted (no matching transition).
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        if self.head.is_empty() {
            return Ok(false); // Halted
        }

        // 1. Find the current symbol under the head
        // Join head and tape on `pos`
        let head_on_tape = self.head.join(&self.tape)?;

        let current_symbol_rel = if head_on_tape.is_empty() {
            // Read blank symbol
            let mut temp = self.head.clone();
            let blank = self.blank_symbol.clone();
            temp = temp
                .extend("symbol", ScalarType::String, move |_| {
                    ScalarValue::String(blank.clone())
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            temp
        } else {
            head_on_tape
        };

        // current_symbol_rel schema: (pos, state, symbol)

        // 2. Prepare transitions for join
        // Rename current_state -> state, read_symbol -> symbol
        let renamed_transitions = self
            .transitions
            .rename(&[("current_state", "state"), ("read_symbol", "symbol")]);

        // 3. Find active transition
        // Join with transitions to find what to do
        let active_transition = current_symbol_rel.join(&renamed_transitions)?;

        if active_transition.is_empty() {
            // Halt: No transition defined for current state and symbol
            return Ok(false);
        }

        // 4. Calculate new head position and state
        let new_head = active_transition
            .extend("new_pos", ScalarType::Int, |t| {
                let p = t.get_typed::<i64>("pos").unwrap();
                let dir = t.get_typed::<i64>("move_dir").unwrap();
                ScalarValue::Int(p + dir)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["new_pos", "next_state"])
            .rename(&[("new_pos", "pos"), ("next_state", "state")]);

        // 5. Update tape
        // The new tape is: all tape cells NOT at the current position UNION the newly written cell
        let current_pos_rel = active_transition.project(&["pos"]);

        // Cells to keep: tape MINUS (tape semi-joined with current position)
        // Since we don't have semijoin implemented explicitly as an operator,
        // we can use join and project, or just restrict based on pos.
        // Or simpler: Difference with the tape cell at the current position.

        // First, extract the old cell to remove
        let old_cell = self.tape.join(&current_pos_rel)?;

        // Tape minus the old cell
        let kept_tape = self
            .tape
            .difference(&old_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Extract the new cell to insert
        let new_cell = active_transition
            .project(&["pos", "write_symbol"])
            .rename(&[("write_symbol", "symbol")]);

        let new_tape = kept_tape
            .union(&new_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Apply updates
        self.head = new_head;
        self.tape = new_tape;

        Ok(true)
    }

    /// Runs the machine until it halts or reaches max_steps.
    pub fn run(&mut self, max_steps: usize) -> Result<usize, DatabaseError> {
        let mut steps = 0;
        for _ in 0..max_steps {
            if !self.step()? {
                return Ok(steps);
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

    #[test]
    fn test_turing_machine_bit_inverter() {
        // A Turing machine that moves right, inverting 0 to 1 and 1 to 0,
        // until it hits a blank 'B', then halts.

        let tape = TuringMachine::create_tape("101").unwrap();
        let head = TuringMachine::create_head("q0", 0).unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("current_state".to_string(), ScalarType::String)
            .with_attribute("read_symbol".to_string(), ScalarType::String)
            .with_attribute("next_state".to_string(), ScalarType::String)
            .with_attribute("write_symbol".to_string(), ScalarType::String)
            .with_attribute("move_dir".to_string(), ScalarType::Int);

        let mut transitions = Relation::new(RelationType::new(trans_heading));

        // State q0: read 1, write 0, move right (+1)
        transitions
            .insert(tuple! {
                current_state: "q0",
                read_symbol: "1",
                next_state: "q0",
                write_symbol: "0",
                move_dir: 1i64
            })
            .unwrap();

        // State q0: read 0, write 1, move right (+1)
        transitions
            .insert(tuple! {
                current_state: "q0",
                read_symbol: "0",
                next_state: "q0",
                write_symbol: "1",
                move_dir: 1i64
            })
            .unwrap();

        let mut tm = TuringMachine::new(tape, head, transitions, "B");

        let steps = tm.run(10).unwrap();
        assert_eq!(steps, 3);

        // Check the final tape
        let mut final_tape: Vec<_> = tm.tape.tuples().collect();
        final_tape.sort_by_key(|t| t.get_typed::<i64>("pos").unwrap());

        assert_eq!(final_tape.len(), 3);

        // Original was "101", should now be "010"
        assert_eq!(final_tape[0].get_typed::<String>("symbol").unwrap(), "0");
        assert_eq!(final_tape[1].get_typed::<String>("symbol").unwrap(), "1");
        assert_eq!(final_tape[2].get_typed::<String>("symbol").unwrap(), "0");
    }
}
