//! Relational Turing Machine
//!
//! This module demonstrates how a Turing Machine can be modeled using purely relational
//! algebra. The machine's tape, head position, and transition function are all relations,
//! and computing the next state is performed entirely via relational operators (Join,
//! Extend, Restrict, Union).
//!
//! # Concept
//!
//! - **Tape**: Relation `(pos: Int, symbol: String)`.
//! - **Head/State**: Relation `(state: String, pos: Int)`. (Cardinality = 1).
//! - **Transitions**: Relation `(current_state: String, read_symbol: String, next_state: String, write_symbol: String, move_dir: Int)`.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Turing Machine.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::turing::TuringMachine;
/// // Note: This is a placeholder example
/// ```
pub struct TuringMachine {
    /// The tape of the Turing Machine. Schema: (pos: Int, symbol: String)
    pub tape: Relation,
    /// The current head and state. Schema: (state: String, pos: Int)
    pub head: Relation,
    /// The transition function.
    /// Schema: (current_state: String, read_symbol: String, next_state: String, write_symbol: String, move_dir: Int)
    pub transitions: Relation,
    /// The symbol used for blank tape cells.
    pub blank_symbol: String,
}

impl TuringMachine {
    /// Creates a new Relational Turing Machine.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::turing::TuringMachine;
    /// // Note: This is a placeholder example
    /// ```
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

    /// Computes the next step of the Turing machine.
    /// Evaluates the machine step and yields `true` if it progressed, or `false` if it halted (no matching transition).
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::turing::TuringMachine;
    /// // Note: This is a placeholder example
    /// ```
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        let current_config = self.get_current_configuration()?;

        // 2. Find the transition to apply by joining with `transitions`.
        // Result heading: (current_state, read_symbol, pos, next_state, write_symbol, move_dir)
        let matched_transition = current_config.join(&self.transitions)?;

        if matched_transition.cardinality() == 0 {
            // No matching transition found; machine halts.
            return Ok(false);
        }

        self.update_tape(&matched_transition)?;
        self.update_head(&matched_transition)?;

        Ok(true)
    }

    fn get_current_configuration(&self) -> Result<Relation, DatabaseError> {
        // 1. Read the symbol under the head.
        // We join `head` (state, pos) with `tape` (pos, symbol).
        // If the pos is not on the tape, it's a blank symbol.
        let read_cell = self.head.join(&self.tape)?;

        let current_symbol_rel = if read_cell.cardinality() == 0 {
            // Head is pointing to a blank cell not explicitly stored in `tape`.
            // We extend `head` with the blank symbol.
            let blank = self.blank_symbol.clone();
            self.head
                .extend("symbol", ScalarType::String, move |_t: &Tuple| {
                    ScalarValue::String(blank.clone())
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        } else {
            read_cell
        };

        // At this point, `current_symbol_rel` has heading (state, pos, symbol) with exactly 1 tuple.
        // Rename `state` -> `current_state` and `symbol` -> `read_symbol` to match `transitions`.
        Ok(current_symbol_rel
            .rename(&[("state", "current_state")])
            .rename(&[("symbol", "read_symbol")]))
    }

    fn update_tape(&mut self, matched_transition: &Relation) -> Result<(), DatabaseError> {
        // 3. Update the tape.
        // We need to write `write_symbol` at `pos`.
        // First, extract the new symbol to write.
        let cell_to_write = matched_transition
            .project(&["pos", "write_symbol"])
            .rename(&[("write_symbol", "symbol")]);

        // Remove the old symbol at `pos` from the tape.
        // We can do this by restricting the tape to `pos` NOT equal to the current head `pos`.
        // Since we know the head `pos` from `matched_transition`, we can join and find the difference.

        let old_tape_cell = matched_transition.project(&["pos"]).join(&self.tape)?;
        let tape_without_old_cell = self
            .tape
            .difference(&old_tape_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Now union the tape without the old cell with the newly written cell.
        self.tape = tape_without_old_cell
            .union(&cell_to_write)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }

    fn update_head(&mut self, matched_transition: &Relation) -> Result<(), DatabaseError> {
        // 4. Update the head position and state.
        // Head needs (state, pos).
        // The new pos = old pos + move_dir.
        let new_head = matched_transition
            .extend("new_pos", ScalarType::Int, |t: &Tuple| {
                let pos = t.get_typed::<i64>("pos").unwrap();
                let dir = t.get_typed::<i64>("move_dir").unwrap();
                ScalarValue::Int(pos + dir)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["next_state", "new_pos"])
            .rename(&[("next_state", "state")])
            .rename(&[("new_pos", "pos")]);

        self.head = new_head;
        Ok(())
    }

    /// Runs the machine until it halts (returns the number of steps taken).
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::turing::TuringMachine;
    /// // Note: This is a placeholder example
    /// ```
    pub fn run(&mut self, max_steps: usize) -> Result<usize, DatabaseError> {
        for step in 0..max_steps {
            if !self.step()? {
                return Ok(step);
            }
        }
        Ok(max_steps)
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::values::Tuple;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    #[test]
    fn test_busy_beaver_2_state() {
        // 2-state, 2-symbol busy beaver.
        // States: "A", "B", "HALT"
        // Alphabet: "0" (blank), "1"

        // Transitions:
        // A, 0 -> B, 1, R
        // A, 1 -> B, 1, L
        // B, 0 -> A, 1, L
        // B, 1 -> HALT, 1, R  (We model halt implicitly by having no transition for "HALT")

        let tape_heading = TupleType::new()
            .with_attribute("pos".to_string(), ScalarType::Int)
            .with_attribute("symbol".to_string(), ScalarType::String);
        let tape = Relation::new(RelationType::new(tape_heading)); // Initially blank

        let head_heading = TupleType::new()
            .with_attribute("state".to_string(), ScalarType::String)
            .with_attribute("pos".to_string(), ScalarType::Int);
        let mut head = Relation::new(RelationType::new(head_heading));
        head.insert(tuple! { state: "A", pos: 0i64 }).unwrap();

        let trans_heading = TupleType::new()
            .with_attribute("current_state".to_string(), ScalarType::String)
            .with_attribute("read_symbol".to_string(), ScalarType::String)
            .with_attribute("next_state".to_string(), ScalarType::String)
            .with_attribute("write_symbol".to_string(), ScalarType::String)
            .with_attribute("move_dir".to_string(), ScalarType::Int);
        let mut transitions = Relation::new(RelationType::new(trans_heading));

        transitions.insert(tuple! { current_state: "A", read_symbol: "0", next_state: "B", write_symbol: "1", move_dir: 1i64 }).unwrap();
        transitions.insert(tuple! { current_state: "A", read_symbol: "1", next_state: "B", write_symbol: "1", move_dir: -1i64 }).unwrap();
        transitions.insert(tuple! { current_state: "B", read_symbol: "0", next_state: "A", write_symbol: "1", move_dir: -1i64 }).unwrap();
        // B, 1 -> HALT (no transition)

        let mut tm = TuringMachine::new(tape, head, transitions, "0".to_string());

        // 2-state BB runs for 6 steps and writes four 1s.
        let steps = tm.run(10).unwrap();
        assert_eq!(steps, 5);

        // Count 1s on tape (ignoring blanks that might have been explicitly written)
        let ones = tm
            .tape
            .restrict(|t: &Tuple| t.get_typed::<String>("symbol").unwrap() == "1");

        assert_eq!(ones.cardinality(), 4);
    }
}
