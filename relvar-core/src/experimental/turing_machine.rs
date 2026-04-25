use crate::types::{RelationType, ScalarType};
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::BTreeMap;

/// Evaluates a Turing Machine step entirely using relational algebra.
///
/// We represent:
/// 1. Tape as a Relation of {pos: Int, symbol: String}
/// 2. State as a Relation of {state: String, head: Int} (always 1 tuple)
/// 3. Rules as a Relation of {state: String, read: String, write: String, move: Int, next_state: String}
///
/// Returns (Tape, State) after one step. If State is empty, the machine has halted.
pub fn step(tape: &Relation, state: &Relation, rules: &Relation) -> (Relation, Relation) {
    if state.is_empty() {
        return (tape.clone(), state.clone());
    }

    // 1. Rename tape 'pos' to 'head' to join with state
    let tape_renamed = tape.clone().rename_into(&[("pos", "head")]);
    let joined = state.join(&tape_renamed).unwrap();

    // We might be reading a blank cell (not in tape relation). Let's construct the current symbol.
    let mut default_symbol = Relation::new(RelationType::new(
        state
            .relation_type()
            .heading()
            .clone()
            .with_attribute("symbol".to_string(), ScalarType::String),
    ));
    for t in state.tuples() {
        let mut vals = BTreeMap::new();
        vals.insert("state".to_string(), t.get("state").unwrap().clone());
        vals.insert("head".to_string(), t.get("head").unwrap().clone());
        vals.insert("symbol".to_string(), ScalarValue::String("B".to_string()));
        let new_t = Tuple::new(default_symbol.relation_type().heading().clone(), vals).unwrap();
        default_symbol.insert(new_t).unwrap();
    }

    let matched = joined.project(&["state", "head", "symbol"]);

    // The current symbol is the matched symbol, or default ('B') if no match
    let current_symbol = if matched.is_empty() {
        default_symbol
    } else {
        matched
    };

    // 2. Join with rules to find active rule
    let active_rule = current_symbol
        .clone()
        .rename_into(&[("symbol", "read")])
        .join(rules)
        .unwrap();

    // If no rule matches, we halt. We signify this by returning an empty state relation.
    if active_rule.is_empty() {
        let empty_state = Relation::new(state.relation_type().clone());
        return (tape.clone(), empty_state);
    }

    // 3. Update the tape
    // Old cell to remove from tape
    let old_cell = current_symbol
        .project(&["head", "symbol"])
        .rename_into(&[("head", "pos")]);
    let tape_no_old = tape.difference(&old_cell).unwrap();

    // New cell to insert into tape
    let new_cell = active_rule
        .project(&["head", "write"])
        .rename_into(&[("head", "pos"), ("write", "symbol")]);
    let new_tape = tape_no_old.union(&new_cell).unwrap();

    // 4. Update the state
    let new_state_extended = active_rule
        .extend_into("new_head", ScalarType::Int, |t: &Tuple| {
            let head = t.get_typed::<i64>("head").unwrap();
            let mov = t.get_typed::<i64>("move").unwrap();
            ScalarValue::Int(head + mov)
        })
        .unwrap();

    let new_state = new_state_extended
        .project(&["next_state", "new_head"])
        .rename_into(&[("next_state", "state"), ("new_head", "head")]);

    (new_tape, new_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::TupleType;

    #[test]
    fn test_turing_machine_step() {
        // Simple Busy Beaver (2-state, 2-symbol)
        // Tape: {pos: Int, symbol: String}
        let tape_type = RelationType::new(
            TupleType::new()
                .with_attribute("pos".to_string(), ScalarType::Int)
                .with_attribute("symbol".to_string(), ScalarType::String),
        );
        let tape = Relation::new(tape_type);
        // Start with blank tape

        // State: {state: String, head: Int}
        let state_type = RelationType::new(
            TupleType::new()
                .with_attribute("state".to_string(), ScalarType::String)
                .with_attribute("head".to_string(), ScalarType::Int),
        );
        let mut state = Relation::new(state_type);
        state
            .insert(tuple! {state: "A".to_string(), head: 0i64})
            .unwrap();

        // Rules: {state: String, read: String, write: String, move: Int, next_state: String}
        // move: -1 (L), 1 (R)
        let rule_type = RelationType::new(
            TupleType::new()
                .with_attribute("state".to_string(), ScalarType::String)
                .with_attribute("read".to_string(), ScalarType::String)
                .with_attribute("write".to_string(), ScalarType::String)
                .with_attribute("move".to_string(), ScalarType::Int)
                .with_attribute("next_state".to_string(), ScalarType::String),
        );
        let mut rules = Relation::new(rule_type);

        // A, B -> 1, R, B
        rules.insert(tuple!{state: "A".to_string(), read: "B".to_string(), write: "1".to_string(), move: 1i64, next_state: "B".to_string()}).unwrap();
        // A, 1 -> 1, L, B
        rules.insert(tuple!{state: "A".to_string(), read: "1".to_string(), write: "1".to_string(), move: -1i64, next_state: "B".to_string()}).unwrap();
        // B, B -> 1, L, A
        rules.insert(tuple!{state: "B".to_string(), read: "B".to_string(), write: "1".to_string(), move: -1i64, next_state: "A".to_string()}).unwrap();
        // B, 1 -> 1, R, H (Halt)
        rules.insert(tuple!{state: "B".to_string(), read: "1".to_string(), write: "1".to_string(), move: 1i64, next_state: "H".to_string()}).unwrap();

        // Step 1: A, B -> write 1, move R (head 1), next state B
        let (tape_1, state_1) = step(&tape, &state, &rules);
        assert_eq!(tape_1.cardinality(), 1);
        assert_eq!(state_1.cardinality(), 1);
        let s = state_1.tuples().next().unwrap();
        assert_eq!(s.get_typed::<String>("state").unwrap(), "B");
        assert_eq!(s.get_typed::<i64>("head").unwrap(), 1);

        // Step 2: B, B -> write 1, move L (head 0), next state A
        let (tape_2, state_2) = step(&tape_1, &state_1, &rules);
        assert_eq!(tape_2.cardinality(), 2);
        let s = state_2.tuples().next().unwrap();
        assert_eq!(s.get_typed::<String>("state").unwrap(), "A");
        assert_eq!(s.get_typed::<i64>("head").unwrap(), 0);

        // Step 3: A, 1 -> write 1, move L (head -1), next state B
        let (tape_3, state_3) = step(&tape_2, &state_2, &rules);
        assert_eq!(tape_3.cardinality(), 2);
        let s = state_3.tuples().next().unwrap();
        assert_eq!(s.get_typed::<String>("state").unwrap(), "B");
        assert_eq!(s.get_typed::<i64>("head").unwrap(), -1);

        // Step 4: B, B -> write 1, move L (head -2), next state A
        let (tape_4, state_4) = step(&tape_3, &state_3, &rules);
        assert_eq!(tape_4.cardinality(), 3);
        let s = state_4.tuples().next().unwrap();
        assert_eq!(s.get_typed::<String>("state").unwrap(), "A");
        assert_eq!(s.get_typed::<i64>("head").unwrap(), -2);
    }
}
