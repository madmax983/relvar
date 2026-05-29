//! Relational Turing Machine Simulator.
//!
//! This module implements a Turing Machine using purely relational algebra.
//! It proves that the relational algebra engine is Turing Complete!

use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue};

/// Evaluates one step of a Turing Machine purely using relational algebra.
///
/// `tape` has attributes: `pos` (Int), `symbol` (String).
/// `transitions` has attributes: `state` (String), `read_symbol` (String), `write_symbol` (String), `direction` (Int), `next_state` (String).
/// `current` has attributes: `state` (String), `head_pos` (Int).
/// `blank_symbol` is the string representing a blank cell.
///
/// Returns `(new_tape, new_current)` or an error. If no transition is found (e.g., reaching an accept/reject state), it returns `None`.
pub fn turing_machine_step(
    tape: &Relation,
    transitions: &Relation,
    current: &Relation,
    blank_symbol: &str,
) -> Result<Option<(Relation, Relation)>, DatabaseError> {
    // 1. Find the symbol at the current head position.
    // current: (state, head_pos)
    // Rename head_pos to pos to join with tape.
    let current_pos = current.rename(&[("head_pos", "pos")]);

    // Tape might not have a tuple for `pos` if it's implicitly blank.
    // We compute: actual_read = (current_pos JOIN tape) UNION (current_pos MINUS tape.project(pos)) x {symbol: blank}
    let matched_tape = current_pos.join(tape)?;

    let tape_positions = tape.project(&["pos"]);
    let missing_pos = current_pos
        .project(&["pos"])
        .difference(&tape_positions)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let blank_str = blank_symbol.to_string();
    let missing_with_blank = missing_pos
        .extend("symbol", ScalarType::String, move |_| {
            ScalarValue::String(blank_str.clone())
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
    let current_missing = current_pos.join(&missing_with_blank)?;

    let actual_read = matched_tape
        .union(&current_missing)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // actual_read: (state, pos, symbol)
    // 2. Join with transitions
    // transitions: (state, read_symbol, write_symbol, direction, next_state)
    // We rename symbol -> read_symbol to match.
    let read_renamed = actual_read.rename(&[("symbol", "read_symbol")]);

    let applied_transition = read_renamed.join(transitions)?;

    // If no transition applies, the machine halts.
    if applied_transition.cardinality() == 0 {
        return Ok(None);
    }

    // applied_transition: (state, pos, read_symbol, write_symbol, direction, next_state)

    // 3. Update the tape
    // Remove the old symbol at `pos`
    let tape_to_remove = tape.semijoin(&current_pos);
    let tape_without_old = tape
        .difference(&tape_to_remove)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Add the new symbol
    let new_cell = applied_transition
        .project(&["pos", "write_symbol"])
        .rename(&[("write_symbol", "symbol")]);

    let new_tape = tape_without_old
        .union(&new_cell)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 4. Update the current state and head position
    // new_current = (next_state, pos + direction)
    let moved = applied_transition
        .extend("head_pos", ScalarType::Int, |t| {
            let p = t.get_typed::<i64>("pos").unwrap();
            let d = t.get_typed::<i64>("direction").unwrap();
            ScalarValue::Int(p + d)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let new_current = moved
        .project(&["next_state", "head_pos"])
        .rename(&[("next_state", "state")]);

    Ok(Some((new_tape, new_current)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    #[test]
    fn test_turing_machine_busy_beaver_2() {
        // A 2-state Busy Beaver machine.
        // States: "A", "B", "HALT".
        // Blank: "0".
        // Transitions:
        // A, 0 -> 1, Right, B
        // A, 1 -> 1, Left, B
        // B, 0 -> 1, Left, A
        // B, 1 -> 1, Right, HALT

        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("pos", ScalarType::Int)
                .with_attribute("symbol", ScalarType::String),
        );
        let mut tape = Relation::new(tape_type);
        // Initially empty tape, but we don't need to insert anything, it handles blanks!

        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("read_symbol", ScalarType::String)
                .with_attribute("write_symbol", ScalarType::String)
                .with_attribute("direction", ScalarType::Int)
                .with_attribute("next_state", ScalarType::String),
        );
        let mut transitions = Relation::new(trans_type);
        transitions.insert(tuple! { state: "A", read_symbol: "0", write_symbol: "1", direction: 1i64, next_state: "B" }).unwrap();
        transitions.insert(tuple! { state: "A", read_symbol: "1", write_symbol: "1", direction: -1i64, next_state: "B" }).unwrap();
        transitions.insert(tuple! { state: "B", read_symbol: "0", write_symbol: "1", direction: -1i64, next_state: "A" }).unwrap();
        transitions.insert(tuple! { state: "B", read_symbol: "1", write_symbol: "1", direction: 1i64, next_state: "HALT" }).unwrap();

        let curr_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("head_pos", ScalarType::Int),
        );
        let mut current = Relation::new(curr_type);
        current
            .insert(tuple! { state: "A", head_pos: 0i64 })
            .unwrap();

        // Step 1: A reads 0 -> writes 1, moves Right, goes to B
        let res1 = turing_machine_step(&tape, &transitions, &current, "0")
            .unwrap()
            .unwrap();
        tape = res1.0;
        current = res1.1;

        // Check current: state="B", head_pos=1
        let t1 = current.tuples().next().unwrap();
        assert_eq!(t1.get_typed::<String>("state").unwrap(), "B");
        assert_eq!(t1.get_typed::<i64>("head_pos").unwrap(), 1);
        // Check tape: (0, "1")
        assert_eq!(tape.cardinality(), 1);

        // Step 2: B reads 0 (blank at pos 1) -> writes 1, moves Left, goes to A
        let res2 = turing_machine_step(&tape, &transitions, &current, "0")
            .unwrap()
            .unwrap();
        tape = res2.0;
        current = res2.1;

        // Check current: state="A", head_pos=0
        let t2 = current.tuples().next().unwrap();
        assert_eq!(t2.get_typed::<String>("state").unwrap(), "A");
        assert_eq!(t2.get_typed::<i64>("head_pos").unwrap(), 0);
        // Check tape: (0, "1"), (1, "1")
        assert_eq!(tape.cardinality(), 2);

        // Step 3: A reads 1 (at pos 0) -> writes 1, moves Left, goes to B
        let res3 = turing_machine_step(&tape, &transitions, &current, "0")
            .unwrap()
            .unwrap();
        tape = res3.0;
        current = res3.1;

        // Check current: state="B", head_pos=-1
        let t3 = current.tuples().next().unwrap();
        assert_eq!(t3.get_typed::<String>("state").unwrap(), "B");
        assert_eq!(t3.get_typed::<i64>("head_pos").unwrap(), -1);

        // Step 4: B reads 0 (blank at pos -1) -> writes 1, moves Left, goes to A
        let res4 = turing_machine_step(&tape, &transitions, &current, "0")
            .unwrap()
            .unwrap();
        tape = res4.0;
        current = res4.1;

        // Step 5: A reads 0 (blank at pos -2) -> writes 1, moves Right, goes to B
        let res5 = turing_machine_step(&tape, &transitions, &current, "0")
            .unwrap()
            .unwrap();
        tape = res5.0;
        current = res5.1;

        // Step 6: B reads 1 (at pos -1) -> writes 1, moves Right, goes to HALT
        let res6 = turing_machine_step(&tape, &transitions, &current, "0")
            .unwrap()
            .unwrap();
        tape = res6.0;
        current = res6.1;

        // Check current: state="HALT"
        let t6 = current.tuples().next().unwrap();
        assert_eq!(t6.get_typed::<String>("state").unwrap(), "HALT");

        // Step 7: HALT state has no transitions, so it should return None
        let res7 = turing_machine_step(&tape, &transitions, &current, "0").unwrap();
        assert!(res7.is_none());

        // Final tape should have 4 ones. (Busy Beaver 2 max score is 4 ones)
        let ones = tape.restrict(|t| t.get_typed::<String>("symbol").unwrap() == "1");
        assert_eq!(ones.cardinality(), 4);
    }
}
