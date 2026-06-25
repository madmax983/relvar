//! Relational Turing Machine
//!
//! This module implements a Turing Machine completely in pure relational algebra.
use crate::DatabaseError;
use crate::types::ScalarType;
use crate::values::relation::RelationError;
use crate::values::{Relation, ScalarValue, Tuple};

fn map_err<E: std::fmt::Debug>(_e: E) -> DatabaseError {
    DatabaseError::Relation(RelationError)
}

/// Evaluates a single step of a Turing Machine purely using relational algebra.
pub fn step(
    tape: Relation,
    head: Relation,
    transitions: Relation,
) -> Result<(Relation, Relation), DatabaseError> {
    // 1. Find which tape position the head is currently over.
    let head_pos = head.clone().project_into(&["pos"]);

    // Tape symbols at the current head positions (if any)
    let tape_known = tape.clone().join(&head_pos).map_err(map_err)?;

    // Head with known tape symbols: (state, pos, symbol)
    let head_known = head.clone().join(&tape_known).map_err(map_err)?;

    // Head positions without a tape symbol
    let head_has_symbol = head_known.clone().project_into(&["state", "pos"]);
    let head_unknown = head.clone().difference(&head_has_symbol).map_err(map_err)?;

    // Default blank symbol is "_"
    let head_unknown_sym = head_unknown
        .extend_into("symbol", ScalarType::String, |_| {
            ScalarValue::String("_".to_string())
        })
        .map_err(map_err)?;

    // Combine to get current read: (state, pos, symbol)
    let current_read = head_known.union(&head_unknown_sym).map_err(map_err)?;

    // 2. Join with transitions
    // Rename current_read to match transitions
    let current_read =
        current_read.rename_into(&[("state", "current_state"), ("symbol", "read_symbol")]);

    // matched_transitions: (current_state, pos, read_symbol, next_state, write_symbol, move_dir)
    let matched_transitions = current_read.join(&transitions).map_err(map_err)?;

    // If no transitions matched, the machine halts (head becomes empty)
    if matched_transitions.is_empty() {
        // Return empty head to signify halt
        let empty_head = Relation::new(head.relation_type().clone());
        return Ok((tape, empty_head));
    }

    // 3. Compute new head
    let extended = matched_transitions
        .clone()
        .extend_into("next_pos", ScalarType::Int, |t: &Tuple| {
            let pos = t.get_typed::<i64>("pos").unwrap();
            let move_dir = t.get_typed::<i64>("move_dir").unwrap();
            ScalarValue::Int(pos.checked_add(move_dir).unwrap())
        })
        .map_err(map_err)?;

    let new_head = extended
        .project_into(&["next_state", "next_pos"])
        .rename_into(&[("next_state", "state"), ("next_pos", "pos")]);

    // 4. Compute new tape
    let new_tape_write = matched_transitions
        .project_into(&["pos", "write_symbol"])
        .rename_into(&[("write_symbol", "symbol")]);

    // Remove old symbols at head positions
    let tape_without_old = tape.difference(&tape_known).map_err(map_err)?;

    // Add new symbols
    let new_tape = tape_without_old.union(&new_tape_write).map_err(map_err)?;

    Ok((new_tape, new_head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    #[test]
    fn test_turing_machine_busy_beaver_2() {
        // 2-state Busy Beaver
        // A, _ -> B, 1, R (+1)
        // A, 1 -> B, 1, L (-1)
        // B, _ -> A, 1, L (-1)
        // B, 1 -> HALT, 1, R (+1)  (we represent HALT by no transition)

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

        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("current_state", ScalarType::String)
                .with_attribute("read_symbol", ScalarType::String)
                .with_attribute("next_state", ScalarType::String)
                .with_attribute("write_symbol", ScalarType::String)
                .with_attribute("move_dir", ScalarType::Int),
        );

        let mut tape = Relation::new(tape_type);
        let mut head = Relation::new(head_type);
        head.insert(tuple! { state: "A".to_string(), pos: 0i64 })
            .unwrap();

        let mut trans = Relation::new(trans_type);
        trans.insert(tuple! { current_state: "A".to_string(), read_symbol: "_".to_string(), next_state: "B".to_string(), write_symbol: "1".to_string(), move_dir: 1i64 }).unwrap();
        trans.insert(tuple! { current_state: "A".to_string(), read_symbol: "1".to_string(), next_state: "B".to_string(), write_symbol: "1".to_string(), move_dir: -1i64 }).unwrap();
        trans.insert(tuple! { current_state: "B".to_string(), read_symbol: "_".to_string(), next_state: "A".to_string(), write_symbol: "1".to_string(), move_dir: -1i64 }).unwrap();
        // Missing (B, 1) means HALT

        // Run steps
        let mut steps = 0;
        loop {
            if head.is_empty() || steps > 10 {
                break;
            }
            let (next_tape, next_head) = step(tape, head, trans.clone()).unwrap();
            tape = next_tape;
            head = next_head;
            steps += 1;
        }

        assert_eq!(steps, 6); // 2-state BB takes 6 steps to halt
        assert_eq!(tape.cardinality(), 4); // Writes four 1s
    }
}
