//! Relational Turing Machine Simulator.
//!
//! This module implements a Turing Machine purely using relational algebra primitives.

use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue};

/// Evaluates one step of a Turing Machine purely using relational algebra.
///
/// `tape` must have attributes: `position` (Int), `symbol` (String).
/// `head` must have attributes: `state` (String), `position` (Int).
/// `transitions` must have attributes: `state` (String), `read_symbol` (String), `next_state` (String), `write_symbol` (String), `direction` (Int).
///
/// # Returns
/// A tuple containing the new `tape` and the new `head` relations.
pub fn step(
    tape: &Relation,
    head: &Relation,
    transitions: &Relation,
    blank_symbol: &str,
) -> Result<(Relation, Relation), DatabaseError> {
    // 1. Get current symbol under head
    let head_with_tape = head
        .join(tape)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Find if the head is over an uninitialized part of the tape
    let head_pos = head_with_tape.project(&["state", "position"]);
    let head_without_tape = head
        .difference(&head_pos)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Extend missing tape positions with the blank symbol
    let blank_sym = blank_symbol.to_string();
    let head_blank = head_without_tape
        .extend("symbol", ScalarType::String, move |_| {
            ScalarValue::String(blank_sym.clone())
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Combine and rename symbol to read_symbol
    let current_state_symbol = head_with_tape
        .union(&head_blank)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        .rename(&[("symbol", "read_symbol")]);

    // 2. Match with transitions
    let matched_transition = current_state_symbol
        .join(transitions)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // If no transition matched, the machine halts (returns empty head)
    if matched_transition.is_empty() {
        // Return tape as is, but empty head
        let empty_head = head.restrict(|_| false);
        return Ok((tape.clone(), empty_head));
    }

    // 3. Compute new head
    let new_head = matched_transition
        .extend("new_position", ScalarType::Int, |t| {
            let pos = t.get_typed::<i64>("position").unwrap();
            let dir = t.get_typed::<i64>("direction").unwrap();
            ScalarValue::Int(pos + dir)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        .project(&["next_state", "new_position"])
        .rename(&[("next_state", "state"), ("new_position", "position")]);

    // 4. Compute new tape
    let modified_position = matched_transition.project(&["position"]);

    // Remove the old symbol at the modified position
    let replaced_tape_tuple = tape
        .join(&modified_position)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
    let old_tape_kept = tape
        .difference(&replaced_tape_tuple)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Insert the new symbol
    let new_tape_tuple = matched_transition
        .project(&["position", "write_symbol"])
        .rename(&[("write_symbol", "symbol")]);

    let new_tape = old_tape_kept
        .union(&new_tape_tuple)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok((new_tape, new_head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    fn create_tape(symbols: &[(i64, &str)]) -> Relation {
        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("position", ScalarType::Int)
                .with_attribute("symbol", ScalarType::String),
        );
        let mut tape = Relation::new(tape_type);
        for &(pos, sym) in symbols {
            tape.insert(tuple! { position: pos, symbol: sym }).unwrap();
        }
        tape
    }

    fn create_head(state: &str, position: i64) -> Relation {
        let head_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("position", ScalarType::Int),
        );
        let mut head = Relation::new(head_type);
        head.insert(tuple! { state: state, position: position })
            .unwrap();
        head
    }

    fn create_transitions(trans: &[(&str, &str, &str, &str, i64)]) -> Relation {
        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("state", ScalarType::String)
                .with_attribute("read_symbol", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("write_symbol", ScalarType::String)
                .with_attribute("direction", ScalarType::Int),
        );
        let mut transitions = Relation::new(trans_type);
        for &(s, r, ns, w, d) in trans {
            transitions
                .insert(tuple! {
                    state: s,
                    read_symbol: r,
                    next_state: ns,
                    write_symbol: w,
                    direction: d
                })
                .unwrap();
        }
        transitions
    }

    #[test]
    fn test_turing_machine_step() {
        // Busy Beaver 2-state, 2-symbol (BB-2)
        // Tape: entirely '0' (blank)
        // States: 'A', 'B' (and implicit 'H' for halt, not in transitions)
        let mut tape = create_tape(&[]);
        let mut head = create_head("A", 0);
        let transitions = create_transitions(&[
            ("A", "0", "B", "1", 1),  // Write 1, Move Right, State B
            ("A", "1", "B", "1", -1), // Write 1, Move Left, State B
            ("B", "0", "A", "1", -1), // Write 1, Move Left, State A
            ("B", "1", "H", "1", 1),  // Write 1, Move Right, State H (Halt)
        ]);

        let blank_symbol = "0";

        // Step 1: A, 0 -> B, 1, R
        let (new_tape, new_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        tape = new_tape;
        head = new_head;

        assert_eq!(tape.cardinality(), 1); // Wrote '1' at pos 0
        assert_eq!(head.cardinality(), 1); // State B, pos 1

        let t1 = tape.tuples().next().unwrap();
        assert_eq!(t1.get_typed::<i64>("position").unwrap(), 0);
        assert_eq!(t1.get_typed::<String>("symbol").unwrap(), "1");

        let h1 = head.tuples().next().unwrap();
        assert_eq!(h1.get_typed::<String>("state").unwrap(), "B");
        assert_eq!(h1.get_typed::<i64>("position").unwrap(), 1);

        // Step 2: B, 0 (blank) -> A, 1, L
        let (new_tape, new_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        tape = new_tape;
        head = new_head;

        assert_eq!(tape.cardinality(), 2); // Wrote '1' at pos 1, pos 0 is still '1'
        assert_eq!(head.cardinality(), 1); // State A, pos 0

        // Step 3: A, 1 -> B, 1, L
        let (new_tape, new_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        tape = new_tape;
        head = new_head;

        assert_eq!(tape.cardinality(), 2);
        assert_eq!(head.cardinality(), 1); // State B, pos -1

        // Step 4: B, 0 (blank) -> A, 1, L
        let (new_tape, new_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        tape = new_tape;
        head = new_head;

        assert_eq!(tape.cardinality(), 3);
        assert_eq!(head.cardinality(), 1); // State A, pos -2

        // Step 5: A, 0 (blank) -> B, 1, R
        let (new_tape, new_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        tape = new_tape;
        head = new_head;

        assert_eq!(tape.cardinality(), 4);
        assert_eq!(head.cardinality(), 1); // State B, pos -1

        // Step 6: B, 1 -> H, 1, R (Halt)
        let (new_tape, new_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        tape = new_tape;
        head = new_head;

        assert_eq!(tape.cardinality(), 4);
        assert_eq!(head.cardinality(), 1); // State H, pos 0

        let h6 = head.tuples().next().unwrap();
        assert_eq!(h6.get_typed::<String>("state").unwrap(), "H");

        // Step 7: H has no transitions, so it should halt (return empty head)
        let (_, final_head) = step(&tape, &head, &transitions, blank_symbol).unwrap();
        assert!(final_head.is_empty());
    }
}
