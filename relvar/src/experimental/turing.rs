//! Relational Turing Machine
//!
//! This module demonstrates that Relvar's relational algebra is Turing complete
//! by implementing a Turing Machine using pure relational algebra.
//!
//! The machine is modeled with three relations:
//! 1. `tape`: (pos: Int, symbol: String)
//! 2. `head`: (state: String, pos: Int)
//! 3. `transitions`: (current_state: String, read_symbol: String, next_state: String, write_symbol: String, move_dir: Int)
//!
//! A single step of the Turing machine is computed by joining these relations to find the applicable
//! transition, then projecting and extending to compute the new `tape` and `head` relations.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Turing Machine.
pub struct TuringMachine {
    /// (pos: Int, symbol: String)
    pub tape: Relation,
    /// (state: String, pos: Int) - always cardinality 1 or 0 (halt)
    pub head: Relation,
    /// (current_state: String, read_symbol: String, next_state: String, write_symbol: String, move_dir: Int)
    pub transitions: Relation,
    /// The symbol considered to be blank on the tape
    pub blank_symbol: String,
}

impl TuringMachine {
    /// Creates a new TuringMachine.
    pub fn new(
        tape: Relation,
        head: Relation,
        transitions: Relation,
        blank_symbol: String,
    ) -> Self {
        Self {
            tape,
            head,
            transitions,
            blank_symbol,
        }
    }

    /// Executes one step of the Turing machine.
    ///
    /// Returns `true` if a step was taken, `false` if the machine has halted (no transition found).
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        // 1. Find the current symbol under the head
        // Join `head` (state, pos) with `tape` (pos, symbol) -> (state, pos, symbol)
        let current_cell = self.head.join(&self.tape)?;

        // If the current cell isn't on the tape, we treat it as a blank symbol.
        // We'll compute the "effective" current cell.
        let effective_cell = if current_cell.is_empty() {
            // No symbol at current head position, so it's blank.
            // Extend `head` with the blank symbol.
            let blank = self.blank_symbol.clone();
            self.head
                .extend("symbol", ScalarType::String, move |_| {
                    ScalarValue::String(blank.clone())
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        } else {
            current_cell
        };

        if effective_cell.is_empty() {
            // Head is gone, halted.
            return Ok(false);
        }

        // Rename for joining with transitions:
        // state -> current_state
        // symbol -> read_symbol
        let current_config =
            effective_cell.rename(&[("state", "current_state"), ("symbol", "read_symbol")]);

        // 2. Find the applicable transition
        // Join `current_config` (current_state, pos, read_symbol) with `transitions`
        // Result: (current_state, pos, read_symbol, next_state, write_symbol, move_dir)
        let matched_transition = current_config.join(&self.transitions)?;

        if matched_transition.is_empty() {
            // No transition found, machine halts.
            self.head = Relation::new(self.head.relation_type().clone());
            return Ok(false);
        }

        // 3. Compute the new head
        // new_pos = pos + move_dir
        // Keep `next_state` and rename it to `state`
        let new_head = matched_transition
            .extend("new_pos", ScalarType::Int, |t| {
                let p = t.get_typed::<i64>("pos").unwrap();
                let m = t.get_typed::<i64>("move_dir").unwrap();
                ScalarValue::Int(p + m)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["next_state", "new_pos"])
            .rename(&[("next_state", "state"), ("new_pos", "pos")]);

        // 4. Compute the new tape
        // The tape is the old tape MINUS the cell at `pos`, PLUS the new written cell
        let current_pos_only = matched_transition.project(&["pos"]);

        // Find the cell on the old tape that is being overwritten (if it exists)
        let old_cell = self.tape.join(&current_pos_only)?;

        // Remove the old cell from the tape
        let tape_without_old_cell = self
            .tape
            .difference(&old_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Create the new cell to write
        let new_cell = matched_transition
            .project(&["pos", "write_symbol"])
            .rename(&[("write_symbol", "symbol")]);

        // Check if we're writing a blank. We don't necessarily have to omit blanks from the tape relation,
        // but it's cleaner. For simplicity, we just union it.
        // If we wanted to optimize, we'd filter out blank writes.
        let new_tape = tape_without_old_cell
            .union(&new_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Update state
        self.head = new_head;
        self.tape = new_tape;

        Ok(true)
    }

    /// Runs the Turing machine until it halts or hits `max_steps`.
    pub fn run(&mut self, max_steps: usize) -> Result<usize, DatabaseError> {
        for step in 0..max_steps {
            if !self.step()? {
                return Ok(step);
            }
        }
        Ok(max_steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_turing_machine_binary_increment() {
        // Increment a binary number on the tape.
        // Number is stored with least significant bit on the left (at pos 0).
        // Initial tape: "11" (represents 3 in binary if read left-to-right as LSB-first).
        // Will become "001" (represents 4 in binary).

        let tape_heading = TupleType::new()
            .with_attribute("pos", ScalarType::Int)
            .with_attribute("symbol", ScalarType::String);
        let mut tape = Relation::new(RelationType::new(tape_heading));
        tape.insert(tuple! {pos: 0i64, symbol: "1" }).unwrap();
        tape.insert(tuple! {pos: 1i64, symbol: "1" }).unwrap();

        let head_heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("pos", ScalarType::Int);
        let mut head = Relation::new(RelationType::new(head_heading));
        head.insert(tuple! {state: "q0", pos: 0i64 }).unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("current_state", ScalarType::String)
            .with_attribute("read_symbol", ScalarType::String)
            .with_attribute("next_state", ScalarType::String)
            .with_attribute("write_symbol", ScalarType::String)
            .with_attribute("move_dir", ScalarType::Int);
        let mut transitions = Relation::new(RelationType::new(trans_heading));

        // If read 1 in q0, write 0, move right (+1), stay in q0 (carry)
        transitions.insert(tuple! {
            current_state: "q0", read_symbol: "1", next_state: "q0", write_symbol: "0", move_dir: 1i64
        }).unwrap();

        // If read 0 in q0, write 1, move right, go to q_halt
        transitions.insert(tuple! {
            current_state: "q0", read_symbol: "0", next_state: "q_halt", write_symbol: "1", move_dir: 1i64
        }).unwrap();

        // If read blank (B) in q0, write 1, move right, go to q_halt
        transitions.insert(tuple! {
            current_state: "q0", read_symbol: "B", next_state: "q_halt", write_symbol: "1", move_dir: 1i64
        }).unwrap();

        let mut tm = TuringMachine::new(tape, head, transitions, "B".to_string());

        let steps = tm.run(10).unwrap();
        assert_eq!(steps, 3);

        // Check final tape
        assert_eq!(tm.tape.cardinality(), 3);

        let mut symbols = std::collections::HashMap::new();
        for t in tm.tape.tuples() {
            let pos = t.get_typed::<i64>("pos").unwrap();
            let sym = t.get_typed::<String>("symbol").unwrap();
            symbols.insert(pos, sym);
        }

        assert_eq!(symbols.get(&0).unwrap(), "0");
        assert_eq!(symbols.get(&1).unwrap(), "0");
        assert_eq!(symbols.get(&2).unwrap(), "1");

        // Check final head state is empty because it hit a state with no transitions
        assert!(tm.head.is_empty());
    }

    #[test]
    fn test_turing_machine_halt() {
        let tape_heading = TupleType::new()
            .with_attribute("pos", ScalarType::Int)
            .with_attribute("symbol", ScalarType::String);
        let tape = Relation::new(RelationType::new(tape_heading));

        let head_heading = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("pos", ScalarType::Int);
        let mut head = Relation::new(RelationType::new(head_heading));
        head.insert(tuple! {state: "q_start", pos: 0i64 }).unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("current_state", ScalarType::String)
            .with_attribute("read_symbol", ScalarType::String)
            .with_attribute("next_state", ScalarType::String)
            .with_attribute("write_symbol", ScalarType::String)
            .with_attribute("move_dir", ScalarType::Int);
        let transitions = Relation::new(RelationType::new(trans_heading));

        let mut tm = TuringMachine::new(tape, head, transitions, "B".to_string());

        // Should halt immediately because there are no transitions
        let stepped = tm.step().unwrap();
        assert!(!stepped);
    }
}
