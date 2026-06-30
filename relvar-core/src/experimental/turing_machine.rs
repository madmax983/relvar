//! Turing Machine Simulator in Relational Algebra.
//!
//! This module implements a Turing Machine simulator using solely relational operators.

use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue};

/// Evaluates one step of a Turing Machine using pure relational algebra.
///
/// `tape` must have attributes: `position` (Int), `symbol` (String).
/// `transitions` must have attributes: `state` (String), `read_symbol` (String),
/// `next_state` (String), `write_symbol` (String), `direction` (Int: -1 for L, 1 for R, 0 for N).
/// `head` must have attributes: `state` (String), `position` (Int).
/// `blank_symbol` is the string representing a blank cell.
pub fn turing_machine_step(
    tape: Relation,
    transitions: &Relation,
    head: Relation,
    blank_symbol: &str,
) -> Result<(Relation, Relation), DatabaseError> {
    // 1. Find the current symbol under the head
    let head_with_tape = head
        .join(&tape)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // If the head is off the recorded tape, it reads a blank symbol.
    let current_read = if head_with_tape.is_empty() {
        head.clone()
            .extend("symbol", ScalarType::String, move |_| {
                ScalarValue::String(blank_symbol.to_string())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
    } else {
        head_with_tape
    };

    // Rename 'symbol' to 'read_symbol' to join with transitions
    let current_read = current_read.rename_into(&[("symbol", "read_symbol")]);

    // 2. Join with transitions to find the applicable rule
    let rule = current_read
        .join(transitions)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // If no rule matches, the machine halts (returns original tape and head)
    if rule.is_empty() {
        return Ok((tape, head));
    }

    // 3. Update the tape:
    // Remove the old cell at the current position
    let tape_without_current = tape
        .difference(
            &current_read
                .project_into(&["position", "read_symbol"])
                .rename_into(&[("read_symbol", "symbol")]),
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Extract the new symbol to write and its position
    let new_cell = rule
        .clone()
        .project_into(&["position", "write_symbol"])
        .rename_into(&[("write_symbol", "symbol")]);

    // Union to update tape
    let new_tape = tape_without_current
        .union(&new_cell)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 4. Update the head:
    let new_head = rule
        .project_into(&["next_state", "position", "direction"])
        .extend("new_position", ScalarType::Int, |t| {
            let pos = t.get_typed::<i64>("position").unwrap_or(0);
            let dir = t.get_typed::<i64>("direction").unwrap_or(0);
            ScalarValue::Int(pos + dir)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        .project_into(&["next_state", "new_position"])
        .rename_into(&[("next_state", "state"), ("new_position", "position")]);

    Ok((new_tape, new_head))
}

/// Runs a Turing Machine until it halts or reaches max_steps.
pub fn run_turing_machine(
    mut tape: Relation,
    transitions: Relation,
    mut head: Relation,
    blank_symbol: &str,
    max_steps: usize,
) -> Result<(Relation, Relation), DatabaseError> {
    for _ in 0..max_steps {
        let prev_head = head.clone();
        let (next_tape, next_head) = turing_machine_step(tape, &transitions, head, blank_symbol)?;
        tape = next_tape;
        head = next_head;

        // Halting condition: head state did not change/no rule matched
        if head == prev_head {
            break;
        }
    }
    Ok((tape, head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    #[test]
    fn test_turing_machine_invert_bits() {
        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("position", ScalarType::Int)
                .with_attribute("symbol", ScalarType::String),
        );
        let mut tape = Relation::new(tape_type);
        tape.insert(tuple! { position: 0i64, symbol: "0" }).unwrap();
        tape.insert(tuple! { position: 1i64, symbol: "1" }).unwrap();

        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("read_symbol", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("write_symbol", ScalarType::String)
                .with_attribute("direction", ScalarType::Int),
        );
        let mut transitions = Relation::new(trans_type);
        transitions.insert(tuple! { state: "q0", read_symbol: "0", next_state: "q0", write_symbol: "1", direction: 1i64 }).unwrap();
        transitions.insert(tuple! { state: "q0", read_symbol: "1", next_state: "q0", write_symbol: "0", direction: 1i64 }).unwrap();
        transitions.insert(tuple! { state: "q0", read_symbol: "B", next_state: "halt", write_symbol: "B", direction: 0i64 }).unwrap();

        let head_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("position", ScalarType::Int),
        );
        let mut head = Relation::new(head_type);
        head.insert(tuple! { state: "q0", position: 0i64 }).unwrap();

        let (final_tape, final_head) =
            run_turing_machine(tape, transitions, head, "B", 10).unwrap();

        assert_eq!(final_tape.cardinality(), 3);
        assert!(final_tape.contains(&tuple! { position: 0i64, symbol: "1" }));
        assert!(final_tape.contains(&tuple! { position: 1i64, symbol: "0" }));
        assert!(final_tape.contains(&tuple! { position: 2i64, symbol: "B" }));

        let head_tuple = final_head.tuples().next().unwrap();
        assert_eq!(head_tuple.get_typed::<String>("state").unwrap(), "halt");
        assert_eq!(head_tuple.get_typed::<i64>("position").unwrap(), 2);
    }
}
