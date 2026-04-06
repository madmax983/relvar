//! Relational Digital Logic Circuit Simulator
//!
//! This module demonstrates how a logic circuit can be modeled and simulated using
//! purely relational algebra. It maps gates, wires, and truth tables into relations,
//! making clock ticks entirely functional state transformations.
//!
//! # Concept
//!
//! - **Gates**: Relation `(gate_id: String, gate_type: String, in1: String, in2: String, out: String)`
//! - **Wires**: Relation `(wire_id: String, signal: Bool)`
//! - **Truth Tables**: Relation `(gate_type: String, val1: Bool, val2: Bool, out_val: Bool)`

use relvar_core::{
    error::DatabaseError,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::Relation,
};

/// A Relational Digital Logic Circuit Simulator.
///
/// Models a logic circuit where:
/// - Gates are a relation of `(gate_id, gate_type, in1, in2, out)`
/// - Wires are a relation of `(wire_id, signal)`
/// - Truth Tables are a relation of `(gate_type, val1, val2, out_val)`
///
/// Simulating a clock tick is purely relational: joining gates with wire states
/// and truth tables to deduce the next states of output wires.
pub struct LogicCircuit {
    /// Schema: (gate_id: String, gate_type: String, in1: String, in2: String, out: String)
    pub gates: Relation,
    /// Schema: (wire_id: String, signal: Bool)
    pub wires: Relation,
    /// Schema: (gate_type: String, val1: Bool, val2: Bool, out_val: Bool)
    pub truth_tables: Relation,
}

impl LogicCircuit {
    /// Creates a new LogicCircuit.
    pub fn new(gates: Relation, wires: Relation) -> Self {
        Self {
            gates,
            wires,
            truth_tables: build_truth_tables(),
        }
    }

    /// Simulates one propagation tick.
    pub fn tick(&self) -> Result<Relation, DatabaseError> {
        let w1 = self.wires.rename(&[("wire_id", "in1"), ("signal", "val1")]);
        let g1 = self.gates.join(&w1)?;

        let w2 = self.wires.rename(&[("wire_id", "in2"), ("signal", "val2")]);
        let g2 = g1.join(&w2)?;

        let eval = g2.join(&self.truth_tables)?;

        let new_signals = eval
            .project(&["out", "out_val"])
            .rename(&[("out", "wire_id"), ("out_val", "signal")]);

        let new_ids = new_signals.project(&["wire_id"]);
        let old_ids = self.wires.project(&["wire_id"]);

        let untouched_ids = old_ids
            .difference(&new_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let untouched_wires = untouched_ids.join(&self.wires)?;

        let updated_wires = new_signals
            .union(&untouched_wires)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(updated_wires)
    }
}

fn build_truth_tables() -> Relation {
    let heading = TupleType::new()
        .with_attribute("gate_type", ScalarType::String)
        .with_attribute("val1", ScalarType::Bool)
        .with_attribute("val2", ScalarType::Bool)
        .with_attribute("out_val", ScalarType::Bool);
    let mut tt = Relation::new(RelationType::new(heading));

    let rules = vec![
        ("AND", false, false, false),
        ("AND", false, true, false),
        ("AND", true, false, false),
        ("AND", true, true, true),
        ("OR", false, false, false),
        ("OR", false, true, true),
        ("OR", true, false, true),
        ("OR", true, true, true),
        ("XOR", false, false, false),
        ("XOR", false, true, true),
        ("XOR", true, false, true),
        ("XOR", true, true, false),
        ("NOT", false, false, true),
        ("NOT", true, true, false), // val2 is ignored conceptually, but here we require a value. Alternatively handle unary NOT distinctly. Let's make NOT just ignore val2 if modeled carefully, or we can just expect identical val1,val2 for NOT inputs in this simplifed model. For a robust model we would handle unary/binary seperately. For simplicity, we just assume val1 and val2 are identical.
        ("NOT", false, true, true),
        ("NOT", true, false, false),
    ];

    for (g_type, v1, v2, out) in rules {
        tt.insert(tuple! {
            gate_type: g_type,
            val1: v1,
            val2: v2,
            out_val: out
        })
        .unwrap();
    }

    tt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_half_adder() {
        let gate_heading = TupleType::new()
            .with_attribute("gate_id", ScalarType::String)
            .with_attribute("gate_type", ScalarType::String)
            .with_attribute("in1", ScalarType::String)
            .with_attribute("in2", ScalarType::String)
            .with_attribute("out", ScalarType::String);
        let mut gates = Relation::new(RelationType::new(gate_heading));

        // Half Adder:
        // SUM = A XOR B
        // CARRY = A AND B
        gates
            .insert(tuple! { gate_id: "g1", gate_type: "XOR", in1: "A", in2: "B", out: "SUM" })
            .unwrap();
        gates
            .insert(tuple! { gate_id: "g2", gate_type: "AND", in1: "A", in2: "B", out: "CARRY" })
            .unwrap();

        let wire_heading = TupleType::new()
            .with_attribute("wire_id", ScalarType::String)
            .with_attribute("signal", ScalarType::Bool);

        // Test 1: A=1, B=0 => SUM=1, CARRY=0
        let mut wires = Relation::new(RelationType::new(wire_heading.clone()));
        wires.insert(tuple! { wire_id: "A", signal: true }).unwrap();
        wires
            .insert(tuple! { wire_id: "B", signal: false })
            .unwrap();

        let circuit = LogicCircuit::new(gates.clone(), wires);
        let new_wires = circuit.tick().unwrap();

        let sum = new_wires
            .tuples()
            .find(|t| t.get_typed::<String>("wire_id").unwrap() == "SUM")
            .unwrap()
            .get_typed::<bool>("signal")
            .unwrap();
        let carry = new_wires
            .tuples()
            .find(|t| t.get_typed::<String>("wire_id").unwrap() == "CARRY")
            .unwrap()
            .get_typed::<bool>("signal")
            .unwrap();

        assert!(sum);
        assert!(!carry);
    }
}
