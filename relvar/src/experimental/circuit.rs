//! Relational Logic Circuit Simulator
//!
//! This module demonstrates how synchronous digital logic circuits can be
//! evaluated using pure relational algebra. Gates and wires are represented
//! as relations, and each evaluation step simulates exactly one gate propagation
//! delay by joining gates with current wire states.
//!
//! # Concept
//!
//! - **Wires**: Relation `(wire: String, val: Bool)`
//! - **BinaryGates**: Relation `(gate: String, gate_type: String, in1: String, in2: String, out: String)`
//! - **UnaryGates**: Relation `(gate: String, gate_type: String, in_wire: String, out: String)`
//! - **Inputs**: Relation `(wire: String, val: Bool)`
//!
//! At each tick, the new state of the wires is computed by joining the gates
//! with the current wire states and applying the logical operations via `Extend`.
//! The results are unioned with the external inputs to form the next state.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Digital Logic Circuit Simulator.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::circuit::LogicSimulator;
/// // Note: This is a placeholder example
/// ```
pub struct LogicSimulator {
    /// Binary gates in the circuit.
    /// Schema: (gate: String, gate_type: String, in1: String, in2: String, out: String)
    pub binary_gates: Relation,
    /// Unary gates in the circuit.
    /// Schema: (gate: String, gate_type: String, in_wire: String, out: String)
    pub unary_gates: Relation,
}

impl LogicSimulator {
    /// Creates a new logic simulator.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::circuit::LogicSimulator;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(binary_gates: Relation, unary_gates: Relation) -> Self {
        Self {
            binary_gates,
            unary_gates,
        }
    }

    /// Evaluates the circuit for one gate propagation delay (one tick).
    ///
    /// # Arguments
    ///
    /// * `current_wires` - The current states of the wires.
    /// * `inputs` - The external inputs that continually drive their wires.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::circuit::LogicSimulator;
    /// // Note: This is a placeholder example
    /// ```
    pub fn tick(
        &self,
        current_wires: &Relation,
        inputs: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 1. Evaluate Binary Gates
        // binary_gates: (gate, gate_type, in1, in2, out)
        // current_wires: (wire, val)

        let w1 = current_wires.rename(&[("wire", "in1"), ("val", "val1")]);
        let w2 = current_wires.rename(&[("wire", "in2"), ("val", "val2")]);

        let joined1 = self.binary_gates.join(&w1)?;
        let joined2 = joined1.join(&w2)?;

        let eval_binary = joined2
            .extend("out_val", ScalarType::Bool, |t| {
                let tpe = t.get_typed::<String>("gate_type").unwrap();
                let v1 = t.get_typed::<bool>("val1").unwrap();
                let v2 = t.get_typed::<bool>("val2").unwrap();

                let res = match tpe.as_str() {
                    "AND" => v1 && v2,
                    "OR" => v1 || v2,
                    "XOR" => v1 ^ v2,
                    "NAND" => !(v1 && v2),
                    "NOR" => !(v1 || v2),
                    "XNOR" => v1 == v2,
                    _ => false, // unknown gate
                };
                ScalarValue::Bool(res)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["out", "out_val"])
            .rename(&[("out", "wire"), ("out_val", "val")]);

        // 2. Evaluate Unary Gates
        // unary_gates: (gate, gate_type, in_wire, out)

        let w_in = current_wires.rename(&[("wire", "in_wire"), ("val", "val_in")]);
        let joined_unary = self.unary_gates.join(&w_in)?;

        let eval_unary = joined_unary
            .extend("out_val", ScalarType::Bool, |t| {
                let tpe = t.get_typed::<String>("gate_type").unwrap();
                let v_in = t.get_typed::<bool>("val_in").unwrap();

                let res = match tpe.as_str() {
                    "NOT" => !v_in,
                    "BUF" => v_in,
                    _ => v_in, // unknown gate
                };
                ScalarValue::Bool(res)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["out", "out_val"])
            .rename(&[("out", "wire"), ("out_val", "val")]);

        // 3. Combine evaluated gates with inputs
        // Inputs override or persist their driven values.
        let gates_out = eval_binary
            .union(&eval_unary)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // In case an input wire also appears in gates_out (e.g. feedback loop, though normally inputs are purely external),
        // we might get conflicting tuples if the values differ. Usually external inputs just persist.
        // We do a difference to remove any gate output that tries to drive an input wire,
        // so inputs take precedence.
        let input_wire_names = inputs.project(&["wire"]);
        let gates_out_safe = gates_out
            .difference(&gates_out.join(&input_wire_names)?)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let next_wires = gates_out_safe
            .union(inputs)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(next_wires)
    }

    /// Evaluates the combinatorial circuit until it stabilizes.
    ///
    /// Simulates gate propagation delays until the state of all wires stops changing.
    /// Returns an error if it fails to stabilize within `max_ticks` (e.g., due to an oscillator loop).
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::circuit::LogicSimulator;
    /// // Note: This is a placeholder example
    /// ```
    pub fn run_until_stable(
        &self,
        initial_wires: &Relation,
        inputs: &Relation,
        max_ticks: usize,
    ) -> Result<Relation, DatabaseError> {
        let mut wires = initial_wires.clone();

        for _ in 0..max_ticks {
            let next_wires = self.tick(&wires, inputs)?;

            let diff1 = next_wires
                .difference(&wires)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            let diff2 = wires
                .difference(&next_wires)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if diff1.is_empty() && diff2.is_empty() {
                return Ok(next_wires);
            }
            wires = next_wires;
        }

        Err(DatabaseError::AlgebraError(
            "Circuit failed to stabilize within maximum ticks (possible oscillation or too deep)"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_full_adder() {
        let bin_heading = TupleType::new()
            .with_attribute("gate", ScalarType::String)
            .with_attribute("gate_type", ScalarType::String)
            .with_attribute("in1", ScalarType::String)
            .with_attribute("in2", ScalarType::String)
            .with_attribute("out", ScalarType::String);
        let mut binary_gates = Relation::new(RelationType::new(bin_heading));

        let un_heading = TupleType::new()
            .with_attribute("gate", ScalarType::String)
            .with_attribute("gate_type", ScalarType::String)
            .with_attribute("in_wire", ScalarType::String)
            .with_attribute("out", ScalarType::String);
        let unary_gates = Relation::new(RelationType::new(un_heading));

        // Full Adder Logic:
        // S1 = A XOR B
        // C1 = A AND B
        // S = S1 XOR Cin
        // C2 = S1 AND Cin
        // Cout = C1 OR C2
        binary_gates
            .insert(tuple! { gate: "X1", gate_type: "XOR", in1: "A", in2: "B", out: "S1" })
            .unwrap();
        binary_gates
            .insert(tuple! { gate: "A1", gate_type: "AND", in1: "A", in2: "B", out: "C1" })
            .unwrap();
        binary_gates
            .insert(tuple! { gate: "X2", gate_type: "XOR", in1: "S1", in2: "Cin", out: "S" })
            .unwrap();
        binary_gates
            .insert(tuple! { gate: "A2", gate_type: "AND", in1: "S1", in2: "Cin", out: "C2" })
            .unwrap();
        binary_gates
            .insert(tuple! { gate: "O1", gate_type: "OR", in1: "C1", in2: "C2", out: "Cout" })
            .unwrap();

        let sim = LogicSimulator::new(binary_gates, unary_gates);

        // Inputs: A=1, B=1, Cin=1
        let inputs_heading = TupleType::new()
            .with_attribute("wire", ScalarType::String)
            .with_attribute("val", ScalarType::Bool);
        let mut inputs = Relation::new(RelationType::new(inputs_heading));
        inputs.insert(tuple! { wire: "A", val: true }).unwrap();
        inputs.insert(tuple! { wire: "B", val: true }).unwrap();
        inputs.insert(tuple! { wire: "Cin", val: true }).unwrap();

        // Evaluate
        let final_state = sim.run_until_stable(&inputs, &inputs, 10).unwrap();

        // Expected: S = 1, Cout = 1
        let s_val = final_state
            .tuples()
            .find(|t| t.get_typed::<String>("wire").unwrap() == "S")
            .unwrap()
            .get_typed::<bool>("val")
            .unwrap();
        assert!(s_val);

        let cout_val = final_state
            .tuples()
            .find(|t| t.get_typed::<String>("wire").unwrap() == "Cout")
            .unwrap()
            .get_typed::<bool>("val")
            .unwrap();
        assert!(cout_val);
    }

    #[test]
    fn test_oscillator() {
        let bin_heading = TupleType::new()
            .with_attribute("gate", ScalarType::String)
            .with_attribute("gate_type", ScalarType::String)
            .with_attribute("in1", ScalarType::String)
            .with_attribute("in2", ScalarType::String)
            .with_attribute("out", ScalarType::String);
        let binary_gates = Relation::new(RelationType::new(bin_heading));

        let un_heading = TupleType::new()
            .with_attribute("gate", ScalarType::String)
            .with_attribute("gate_type", ScalarType::String)
            .with_attribute("in_wire", ScalarType::String)
            .with_attribute("out", ScalarType::String);
        let mut unary_gates = Relation::new(RelationType::new(un_heading));

        // NOT gate looped back to itself
        unary_gates
            .insert(tuple! { gate: "N1", gate_type: "NOT", in_wire: "CLK", out: "CLK" })
            .unwrap();

        let sim = LogicSimulator::new(binary_gates, unary_gates);

        let inputs_heading = TupleType::new()
            .with_attribute("wire", ScalarType::String)
            .with_attribute("val", ScalarType::Bool);
        let mut initial_state = Relation::new(RelationType::new(inputs_heading.clone()));
        initial_state
            .insert(tuple! { wire: "CLK", val: false })
            .unwrap();

        let empty_inputs = Relation::new(RelationType::new(inputs_heading.clone()));

        let state1 = sim.tick(&initial_state, &empty_inputs).unwrap(); // CLK should be true
        let clk_val1 = state1
            .tuples()
            .find(|t| t.get_typed::<String>("wire").unwrap() == "CLK")
            .unwrap()
            .get_typed::<bool>("val")
            .unwrap();
        assert!(clk_val1);

        let state2 = sim.tick(&state1, &empty_inputs).unwrap(); // CLK should be false
        let clk_val2 = state2
            .tuples()
            .find(|t| t.get_typed::<String>("wire").unwrap() == "CLK")
            .unwrap()
            .get_typed::<bool>("val")
            .unwrap();
        assert!(!clk_val2);

        // run_until_stable should fail

        let res = sim.run_until_stable(&initial_state, &empty_inputs, 10);

        assert!(res.is_err());
    }
}
