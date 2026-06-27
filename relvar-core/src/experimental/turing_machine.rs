use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

/// A Relational Turing Machine modeled purely using relational algebra.
pub struct RelationalTuringMachine {
    /// The transitions relation
    pub transitions: Relation,
    /// The tape relation
    pub tape: Relation,
    /// The head relation
    pub head: Relation,
}

impl RelationalTuringMachine {
    /// Creates a new Relational Turing Machine.
    pub fn new() -> Self {
        let transitions_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("read", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("write", ScalarType::String)
                .with_attribute("move_dir", ScalarType::Int),
        );

        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("pos", ScalarType::Int)
                .with_attribute("symbol", ScalarType::String),
        );

        let head_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("pos", ScalarType::Int),
        );

        Self {
            transitions: Relation::new(transitions_type),
            tape: Relation::new(tape_type),
            head: Relation::new(head_type),
        }
    }

    /// Adds a transition.
    pub fn add_transition(
        &mut self,
        state: &str,
        read: &str,
        next_state: &str,
        write: &str,
        move_dir: i64,
    ) -> Result<(), DatabaseError> {
        let t = crate::tuple! {
            state: state,
            read: read,
            next_state: next_state,
            write: write,
            move_dir: move_dir
        };
        self.transitions.insert(t).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Writes to tape.
    pub fn write_tape(&mut self, pos: i64, symbol: &str) -> Result<(), DatabaseError> {
        let t = crate::tuple! {
            pos: pos,
            symbol: symbol
        };
        self.tape.insert(t).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Sets head.
    pub fn set_head(&mut self, state: &str, pos: i64) -> Result<(), DatabaseError> {
        self.head = Relation::new(self.head.relation_type().clone());
        let t = crate::tuple! {
            state: state,
            pos: pos
        };
        self.head.insert(t).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Steps the machine.
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        let renamed_tape = self.tape.rename(&[("symbol", "read")]);
        let current_read_matches = self.head.join(&renamed_tape).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let tape_pos_only = self.tape.project(&["pos"]);
        let missing_tape = self.head.semidifference(&tape_pos_only);
        let blank_read = missing_tape.extend("read", ScalarType::String, |_| {
            ScalarValue::String("_".to_string())
        }).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let actual_read = current_read_matches.union(&blank_read).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let fired = actual_read.join(&self.transitions).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        if fired.cardinality() == 0 {
            return Ok(false);
        }

        let next_head_ext = fired.extend("next_pos", ScalarType::Int, |t| {
            let pos = t.get_typed::<i64>("pos").unwrap();
            let move_dir = t.get_typed::<i64>("move_dir").unwrap();
            ScalarValue::Int(pos + move_dir)
        }).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let next_head_proj = next_head_ext.project(&["next_state", "next_pos"]);
        let next_head = next_head_proj.rename(&[
            ("next_state", "state"),
            ("next_pos", "pos"),
        ]);

        let fired_pos = fired.project(&["pos"]);
        let unchanged_tape = self.tape.semidifference(&fired_pos);

        let written_cell = fired.project(&["pos", "write"])
            .rename(&[("write", "symbol")]);

        let next_tape = unchanged_tape.union(&written_cell).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.head = next_head;
        self.tape = next_tape;

        Ok(true)
    }
}

impl Default for RelationalTuringMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turing_machine_busy_beaver_2() {
        let mut tm = RelationalTuringMachine::new();

        tm.add_transition("A", "_", "B", "1", 1).unwrap();
        tm.add_transition("A", "1", "B", "1", -1).unwrap();
        tm.add_transition("B", "_", "A", "1", -1).unwrap();
        tm.add_transition("B", "1", "HALT", "1", 1).unwrap();

        tm.set_head("A", 0).unwrap();

        let mut steps = 0;
        while tm.step().unwrap() {
            steps += 1;
            if steps > 20 {
                panic!("Infinite loop detected, took too many steps!");
            }
        }

        assert_eq!(steps, 6);
        assert_eq!(tm.tape.cardinality(), 4);
    }
}
