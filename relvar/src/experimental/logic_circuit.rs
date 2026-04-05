//! Relational Logic Circuit Simulator
//!
//! This module demonstrates how a logic circuit can be modeled and evaluated
//! using purely relational algebra. Gates, wires, and states are all represented
//! as relations.
//!
//! # Concept
//!
//! - **Gates**: Relation `(gate_id: String, type: String)` where `type` is one of "AND", "OR", "NOT", "XOR", "IN".
//! - **Wires**: Relation `(from_gate: String, to_gate: String, to_pin: Int)`.
//! - **State**: Relation `(gate_id: String, value: Bool)`.
//!
//! We compute the next state of the circuit iteratively by joining the current state
//! with wires and gates, grouping inputs by `to_gate`, and applying logical operations
//! using `extend` and `summarize`.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Logic Circuit.
pub struct LogicCircuit {
    /// The gates in the circuit. Schema: (gate_id: String, type: String)
    pub gates: Relation,
    /// The wires connecting gates. Schema: (from_gate: String, to_gate: String, to_pin: Int)
    pub wires: Relation,
    /// The current state of each gate output. Schema: (gate_id: String, value: Bool)
    pub state: Relation,
}

impl LogicCircuit {
    /// Creates a new LogicCircuit from relations.
    pub fn new(gates: Relation, wires: Relation, initial_state: Relation) -> Self {
        Self {
            gates,
            wires,
            state: initial_state,
        }
    }

    /// Evaluates one tick of the circuit.
    pub fn tick(&mut self) -> Result<(), DatabaseError> {
        // 1. Join current state with wires to find inputs arriving at each gate
        // state(gate_id, value)
        // rename to (from_gate, in_value)
        let state_out = self
            .state
            .rename(&[("gate_id", "from_gate"), ("value", "in_value")]);

        // Joined: (from_gate, in_value, to_gate, to_pin)
        let inputs = state_out.join(&self.wires)?;

        // 2. We need to collect inputs per destination gate. Since relations are sets,
        // and a gate can have multiple inputs (pin 0, pin 1), we can't easily fold over rows
        // with pure relational operators unless we have a specific aggregation function for arrays/lists.
        // But for standard binary gates (AND, OR, XOR), we can just sum or count `true` inputs.
        // For NOT, there's only 1 input.
        // Let's compute statistics of inputs per gate.

        // Convert boolean to int to sum them up. (true -> 1, false -> 0)
        let inputs_int = inputs
            .extend("in_int", ScalarType::Int, |t| {
                let val = t.get_typed::<bool>("in_value").unwrap();
                ScalarValue::Int(if val { 1 } else { 0 })
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Count total inputs and sum of true inputs per destination gate
        let gate_inputs_summary = inputs_int
            .summarize(
                &["to_gate"],
                &[
                    Aggregation::count("num_inputs"),
                    Aggregation::sum("true_inputs", "in_int"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Join with Gates to apply logic
        // Rename `to_gate` -> `gate_id` to join with self.gates
        let inputs_for_eval = gate_inputs_summary.rename(&[("to_gate", "gate_id")]);

        let gates_with_inputs = inputs_for_eval.join(&self.gates)?;

        // 4. Apply gate logic based on gate type, num_inputs, and true_inputs
        let new_evaluated_state = gates_with_inputs
            .extend("value", ScalarType::Bool, |t| {
                let gate_type = t.get_typed::<String>("type").unwrap();
                let num_inputs = t.get_typed::<i64>("num_inputs").unwrap();
                let true_inputs = t.get_typed::<i64>("true_inputs").unwrap();

                let val = match gate_type.as_str() {
                    "AND" => true_inputs == num_inputs && num_inputs > 0,
                    "OR" => true_inputs > 0,
                    "XOR" => true_inputs % 2 == 1,
                    "NOT" => true_inputs == 0,
                    _ => false, // fallback, e.g., "IN" gates should not change here if they don't have inputs
                };
                ScalarValue::Bool(val)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["gate_id", "value"]);

        // Keep IN gates states from previous state
        let in_gates = self
            .gates
            .restrict(|t| t.get_typed::<String>("type").unwrap() == "IN")
            .project(&["gate_id"]);
        let in_gates_state = self.state.join(&in_gates)?;

        // The new state is the union of newly evaluated states and the static IN gate states
        self.state = new_evaluated_state
            .union(&in_gates_state)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_logic_circuit_and_gate() {
        let gates_heading = TupleType::new()
            .with_attribute("gate_id", ScalarType::String)
            .with_attribute("type", ScalarType::String);
        let mut gates = Relation::new(RelationType::new(gates_heading));
        gates.insert(tuple! { gate_id: "IN1", type: "IN" }).unwrap();
        gates.insert(tuple! { gate_id: "IN2", type: "IN" }).unwrap();
        gates
            .insert(tuple! { gate_id: "AND1", type: "AND" })
            .unwrap();

        let wires_heading = TupleType::new()
            .with_attribute("from_gate", ScalarType::String)
            .with_attribute("to_gate", ScalarType::String)
            .with_attribute("to_pin", ScalarType::Int);
        let mut wires = Relation::new(RelationType::new(wires_heading));
        wires
            .insert(tuple! { from_gate: "IN1", to_gate: "AND1", to_pin: 0i64 })
            .unwrap();
        wires
            .insert(tuple! { from_gate: "IN2", to_gate: "AND1", to_pin: 1i64 })
            .unwrap();

        let state_heading = TupleType::new()
            .with_attribute("gate_id", ScalarType::String)
            .with_attribute("value", ScalarType::Bool);
        let mut state = Relation::new(RelationType::new(state_heading));
        state
            .insert(tuple! { gate_id: "IN1", value: true })
            .unwrap();
        state
            .insert(tuple! { gate_id: "IN2", value: true })
            .unwrap();

        let mut circuit = LogicCircuit::new(gates, wires, state);

        circuit.tick().unwrap();

        let and1_state = circuit
            .state
            .tuples()
            .find(|t| t.get_typed::<String>("gate_id").unwrap() == "AND1")
            .unwrap();
        assert!(and1_state.get_typed::<bool>("value").unwrap());
    }

    #[test]
    fn test_logic_circuit_half_adder() {
        // Half Adder:
        // S = A XOR B
        // C = A AND B

        let gates_heading = TupleType::new()
            .with_attribute("gate_id", ScalarType::String)
            .with_attribute("type", ScalarType::String);
        let mut gates = Relation::new(RelationType::new(gates_heading));
        gates.insert(tuple! { gate_id: "A", type: "IN" }).unwrap();
        gates.insert(tuple! { gate_id: "B", type: "IN" }).unwrap();
        gates
            .insert(tuple! { gate_id: "XOR1", type: "XOR" })
            .unwrap();
        gates
            .insert(tuple! { gate_id: "AND1", type: "AND" })
            .unwrap();

        let wires_heading = TupleType::new()
            .with_attribute("from_gate", ScalarType::String)
            .with_attribute("to_gate", ScalarType::String)
            .with_attribute("to_pin", ScalarType::Int);
        let mut wires = Relation::new(RelationType::new(wires_heading));
        wires
            .insert(tuple! { from_gate: "A", to_gate: "XOR1", to_pin: 0i64 })
            .unwrap();
        wires
            .insert(tuple! { from_gate: "B", to_gate: "XOR1", to_pin: 1i64 })
            .unwrap();
        wires
            .insert(tuple! { from_gate: "A", to_gate: "AND1", to_pin: 0i64 })
            .unwrap();
        wires
            .insert(tuple! { from_gate: "B", to_gate: "AND1", to_pin: 1i64 })
            .unwrap();

        let state_heading = TupleType::new()
            .with_attribute("gate_id", ScalarType::String)
            .with_attribute("value", ScalarType::Bool);
        let mut state = Relation::new(RelationType::new(state_heading));
        // A = 1, B = 0 -> S = 1, C = 0
        state.insert(tuple! { gate_id: "A", value: true }).unwrap();
        state.insert(tuple! { gate_id: "B", value: false }).unwrap();

        let mut circuit = LogicCircuit::new(gates, wires, state);

        circuit.tick().unwrap();

        let sum_state = circuit
            .state
            .tuples()
            .find(|t| t.get_typed::<String>("gate_id").unwrap() == "XOR1")
            .unwrap();
        assert!(sum_state.get_typed::<bool>("value").unwrap());

        let carry_state = circuit
            .state
            .tuples()
            .find(|t| t.get_typed::<String>("gate_id").unwrap() == "AND1")
            .unwrap();
        assert!(!carry_state.get_typed::<bool>("value").unwrap());
    }
}
