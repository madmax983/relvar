use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

/// A Turing Machine implemented purely with relational algebra.
///
/// We simulate a Turing Machine where:
/// - Tape is a relation of (pos: Int, symbol: String)
/// - State is a relation of (state: String, pos: Int)
/// - Rules is a relation of (state: String, symbol: String, next_state: String, write_symbol: String, move_dir: Int)
///
/// The machine operates by iteratively joining the State, Tape, and Rules relations to find the next state and tape modifications.
pub struct TuringMachine {
    /// The tape of the Turing machine.
    pub tape: Relation,
    /// The current state of the Turing machine.
    pub state: Relation,
    /// The transition rules of the Turing machine.
    pub rules: Relation,
    /// The blank symbol used by the Turing machine.
    pub blank_symbol: String,
}

impl TuringMachine {
    /// Create a new Turing Machine with the given blank symbol.
    pub fn new(blank_symbol: &str) -> Self {
        let tape_type = TupleType::new()
            .with_attribute("pos", ScalarType::Int)
            .with_attribute("symbol", ScalarType::String);

        let state_type = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("pos", ScalarType::Int);

        let rules_type = TupleType::new()
            .with_attribute("state", ScalarType::String)
            .with_attribute("symbol", ScalarType::String)
            .with_attribute("next_state", ScalarType::String)
            .with_attribute("write_symbol", ScalarType::String)
            .with_attribute("move_dir", ScalarType::Int);

        Self {
            tape: Relation::new(RelationType::new(tape_type)),
            state: Relation::new(RelationType::new(state_type)),
            rules: Relation::new(RelationType::new(rules_type)),
            blank_symbol: blank_symbol.to_string(),
        }
    }

    /// Run the Turing Machine for one step.
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        // Find current symbol under the head by joining state and tape on 'pos'
        let current_tape = self.tape.clone();
        let mut head = self
            .state
            .join(&current_tape)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // If head is empty, we might be on a blank cell. We can simulate reading a blank.
        if head.cardinality() == 0 {
            // we extend the state with a blank symbol
            let blank = self.blank_symbol.clone();
            head = self
                .state
                .extend("symbol", ScalarType::String, move |_| {
                    ScalarValue::String(blank.clone())
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        }

        // Find the applicable rule by joining head with rules on 'state' and 'symbol'
        let applicable_rule = head
            .join(&self.rules)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        if applicable_rule.cardinality() == 0 {
            // Halt
            return Ok(false);
        }

        // 1. Update Tape: The cell at 'pos' is replaced by 'write_symbol'
        // Project to get the cell to replace
        let to_replace = applicable_rule.project(&["pos", "symbol"]);
        self.tape = self
            .tape
            .difference(&to_replace)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // New cell to insert
        let new_cell_temp = applicable_rule.project(&["pos", "write_symbol"]);
        let new_cell = new_cell_temp.rename(&[("write_symbol", "symbol")]);
        self.tape = self
            .tape
            .union(&new_cell)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Update State: The head moves to 'pos' + 'move_dir' and changes to 'next_state'
        let next_state_temp = applicable_rule
            .extend("new_pos", ScalarType::Int, |t| {
                let pos = t.get_typed::<i64>("pos").unwrap_or(0);
                let move_dir = t.get_typed::<i64>("move_dir").unwrap_or(0);
                ScalarValue::Int(pos + move_dir)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let next_state_proj = next_state_temp.project(&["next_state", "new_pos"]);
        self.state = next_state_proj.rename(&[("next_state", "state"), ("new_pos", "pos")]);

        Ok(true)
    }

    /// Run the Turing Machine for at most `max_steps`. Returns the number of steps executed.
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
    use crate::tuple;

    #[test]
    fn test_turing_machine() {
        let mut tm = TuringMachine::new("_");

        // Write some '1's on the tape
        let _ = tm
            .tape
            .insert(tuple! { pos: 0i64, symbol: "1".to_string() });
        let _ = tm
            .tape
            .insert(tuple! { pos: 1i64, symbol: "1".to_string() });
        let _ = tm
            .tape
            .insert(tuple! { pos: 2i64, symbol: "1".to_string() });

        // Initial state at position 0
        let _ = tm
            .state
            .insert(tuple! { state: "q0".to_string(), pos: 0i64 });

        // Rule: if in q0, read 1, write 0, move right, stay in q0
        let _ = tm.rules.insert(tuple! {
            state: "q0".to_string(),
            symbol: "1".to_string(),
            next_state: "q0".to_string(),
            write_symbol: "0".to_string(),
            move_dir: 1i64
        });

        // Rule: if in q0, read blank (_), halt (no rule matches, or transition to halt)

        tm.run(10).unwrap();

        // Tape should have 0s at 0, 1, 2
        let pos_0 = tm
            .tape
            .restrict(|t| t.get_typed::<i64>("pos").unwrap() == 0);
        assert_eq!(
            pos_0
                .tuples()
                .next()
                .unwrap()
                .get_typed::<String>("symbol")
                .unwrap(),
            "0"
        );

        let pos_1 = tm
            .tape
            .restrict(|t| t.get_typed::<i64>("pos").unwrap() == 1);
        assert_eq!(
            pos_1
                .tuples()
                .next()
                .unwrap()
                .get_typed::<String>("symbol")
                .unwrap(),
            "0"
        );

        let pos_2 = tm
            .tape
            .restrict(|t| t.get_typed::<i64>("pos").unwrap() == 2);
        assert_eq!(
            pos_2
                .tuples()
                .next()
                .unwrap()
                .get_typed::<String>("symbol")
                .unwrap(),
            "0"
        );
    }
}
