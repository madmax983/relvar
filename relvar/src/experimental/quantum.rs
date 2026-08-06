//! Relational Quantum Circuit Simulator.
//!
//! This module demonstrates how quantum circuits and state vectors can be
//! modeled using purely relational algebra. A quantum state is a relation where
//! each tuple represents a basis state and its complex probability amplitude.
//! Quantum gates are modeled as transition relations, and gate application is
//! equivalent to relational joins (tensor contraction) followed by aggregation.

use relvar_core::algebra::Aggregation;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::BTreeMap;

/// A Relational Quantum Circuit Simulator.
pub struct QuantumCircuit {
    /// The current quantum state vector as a relation.
    /// Attributes: `q0`, `q1`, ..., `qN` (Int), `real` (Float), `imag` (Float).
    pub state: Relation,
    /// Number of qubits in the circuit.
    pub num_qubits: usize,
}

impl QuantumCircuit {
    /// Creates a new quantum circuit with `num_qubits` initialized to the |0...0> state.
    pub fn new(num_qubits: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut heading = TupleType::new();
        for i in 0..num_qubits {
            heading = heading.with_attribute(format!("q{}", i), ScalarType::Int);
        }
        heading = heading.with_attribute("real".to_string(), ScalarType::Float);
        heading = heading.with_attribute("imag".to_string(), ScalarType::Float);

        let mut state = Relation::new(RelationType::new(heading.clone()));

        // Create the |0...0> state
        let mut initial_state = BTreeMap::new();
        for i in 0..num_qubits {
            initial_state.insert(format!("q{}", i), ScalarValue::Int(0));
        }
        initial_state.insert("real".to_string(), ScalarValue::Float(1.0));
        initial_state.insert("imag".to_string(), ScalarValue::Float(0.0));

        state.insert(Tuple::new(heading.clone(), initial_state).unwrap())?;

        Ok(Self { state, num_qubits })
    }

    /// Applies a 1-qubit gate to the specified qubit.
    pub fn apply_1q_gate(
        &mut self,
        target_qubit: usize,
        gate_matrix: &[(i64, i64, f64, f64)],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let q_attr = format!("q{}", target_qubit);
        let in_attr = format!("in_q{}", target_qubit);
        let out_attr = format!("out_q{}", target_qubit);

        // 1. Rename the target qubit column to `in_attr`
        let current_state = self.state.clone().rename(&[(&q_attr, &in_attr)]);

        // 2. Create the gate relation
        let gate_heading = TupleType::new()
            .with_attribute(in_attr.clone(), ScalarType::Int)
            .with_attribute(out_attr.clone(), ScalarType::Int)
            .with_attribute("gate_r".to_string(), ScalarType::Float)
            .with_attribute("gate_i".to_string(), ScalarType::Float);

        let mut gate_rel = Relation::new(RelationType::new(gate_heading.clone()));
        for &(in_val, out_val, r, i) in gate_matrix {
            let mut tuple_map = BTreeMap::new();
            tuple_map.insert(in_attr.clone(), ScalarValue::Int(in_val));
            tuple_map.insert(out_attr.clone(), ScalarValue::Int(out_val));
            tuple_map.insert("gate_r".to_string(), ScalarValue::Float(r));
            tuple_map.insert("gate_i".to_string(), ScalarValue::Float(i));
            gate_rel.insert(Tuple::new(gate_heading.clone(), tuple_map).unwrap())?;
        }

        // 3. Join state with gate
        let joined = current_state.join(&gate_rel)?;

        // 4. Extend to compute new amplitudes
        let extended_r = joined.extend("new_r", ScalarType::Float, |t: &Tuple| {
            let a = t.get_typed::<f64>("real").unwrap_or(0.0);
            let b = t.get_typed::<f64>("imag").unwrap_or(0.0);
            let c = t.get_typed::<f64>("gate_r").unwrap_or(0.0);
            let d = t.get_typed::<f64>("gate_i").unwrap_or(0.0);
            ScalarValue::Float(a * c - b * d)
        })?;

        let extended_i = extended_r.extend("new_i", ScalarType::Float, |t: &Tuple| {
            let a = t.get_typed::<f64>("real").unwrap_or(0.0);
            let b = t.get_typed::<f64>("imag").unwrap_or(0.0);
            let c = t.get_typed::<f64>("gate_r").unwrap_or(0.0);
            let d = t.get_typed::<f64>("gate_i").unwrap_or(0.0);
            ScalarValue::Float(a * d + b * c)
        })?;

        // 5. Summarize over all qubits
        let mut group_attrs = Vec::new();
        for i in 0..self.num_qubits {
            if i == target_qubit {
                group_attrs.push(out_attr.clone());
            } else {
                group_attrs.push(format!("q{}", i));
            }
        }

        let aggs = vec![
            Aggregation::sum_float("sum_r", "new_r"),
            Aggregation::sum_float("sum_i", "new_i"),
        ];

        let grouped_attrs: Vec<&str> = group_attrs.iter().map(|s| s.as_str()).collect();
        let summarized = extended_i.summarize(&grouped_attrs, &aggs)?;

        // 6. Rename output back to state format
        self.state =
            summarized.rename(&[(&out_attr, &q_attr), ("sum_r", "real"), ("sum_i", "imag")]);
        Ok(())
    }

    /// Applies a Hadamard gate.
    pub fn h(&mut self, target: usize) -> Result<(), Box<dyn std::error::Error>> {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        self.apply_1q_gate(
            target,
            &[
                (0, 0, s, 0.0),
                (0, 1, s, 0.0),
                (1, 0, s, 0.0),
                (1, 1, -s, 0.0),
            ],
        )
    }

    /// Applies a Pauli-X (NOT) gate.
    pub fn x(&mut self, target: usize) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_1q_gate(target, &[(0, 1, 1.0, 0.0), (1, 0, 1.0, 0.0)])
    }

    /// Applies a CNOT gate.
    pub fn cx(&mut self, control: usize, target: usize) -> Result<(), Box<dyn std::error::Error>> {
        let ctrl_attr = format!("q{}", control);
        let tgt_attr = format!("q{}", target);
        let in_c = format!("in_q{}", control);
        let in_t = format!("in_q{}", target);
        let out_c = format!("out_q{}", control);
        let out_t = format!("out_q{}", target);

        let current_state = self
            .state
            .clone()
            .rename(&[(&ctrl_attr, &in_c), (&tgt_attr, &in_t)]);

        let gate_heading = TupleType::new()
            .with_attribute(in_c.clone(), ScalarType::Int)
            .with_attribute(in_t.clone(), ScalarType::Int)
            .with_attribute(out_c.clone(), ScalarType::Int)
            .with_attribute(out_t.clone(), ScalarType::Int)
            .with_attribute("gate_r".to_string(), ScalarType::Float)
            .with_attribute("gate_i".to_string(), ScalarType::Float);

        let mut gate_rel = Relation::new(RelationType::new(gate_heading.clone()));
        let gate_matrix = [
            (0, 0, 0, 0, 1.0, 0.0),
            (0, 1, 0, 1, 1.0, 0.0),
            (1, 0, 1, 1, 1.0, 0.0),
            (1, 1, 1, 0, 1.0, 0.0),
        ];

        for &(ic, it, oc, ot, r, i) in &gate_matrix {
            let mut tuple_map = BTreeMap::new();
            tuple_map.insert(in_c.clone(), ScalarValue::Int(ic));
            tuple_map.insert(in_t.clone(), ScalarValue::Int(it));
            tuple_map.insert(out_c.clone(), ScalarValue::Int(oc));
            tuple_map.insert(out_t.clone(), ScalarValue::Int(ot));
            tuple_map.insert("gate_r".to_string(), ScalarValue::Float(r));
            tuple_map.insert("gate_i".to_string(), ScalarValue::Float(i));
            gate_rel.insert(Tuple::new(gate_heading.clone(), tuple_map).unwrap())?;
        }

        let joined = current_state.join(&gate_rel)?;

        let extended_r = joined.extend("new_r", ScalarType::Float, |t: &Tuple| {
            let a = t.get_typed::<f64>("real").unwrap_or(0.0);
            let b = t.get_typed::<f64>("imag").unwrap_or(0.0);
            let c = t.get_typed::<f64>("gate_r").unwrap_or(0.0);
            let d = t.get_typed::<f64>("gate_i").unwrap_or(0.0);
            ScalarValue::Float(a * c - b * d)
        })?;

        let extended_i = extended_r.extend("new_i", ScalarType::Float, |t: &Tuple| {
            let a = t.get_typed::<f64>("real").unwrap_or(0.0);
            let b = t.get_typed::<f64>("imag").unwrap_or(0.0);
            let c = t.get_typed::<f64>("gate_r").unwrap_or(0.0);
            let d = t.get_typed::<f64>("gate_i").unwrap_or(0.0);
            ScalarValue::Float(a * d + b * c)
        })?;

        let mut group_attrs = Vec::new();
        for i in 0..self.num_qubits {
            if i == control {
                group_attrs.push(out_c.clone());
            } else if i == target {
                group_attrs.push(out_t.clone());
            } else {
                group_attrs.push(format!("q{}", i));
            }
        }

        let aggs = vec![
            Aggregation::sum_float("sum_r", "new_r"),
            Aggregation::sum_float("sum_i", "new_i"),
        ];

        let grouped_attrs: Vec<&str> = group_attrs.iter().map(|s| s.as_str()).collect();
        let summarized = extended_i.summarize(&grouped_attrs, &aggs)?;

        self.state = summarized.rename(&[
            (&out_c, &ctrl_attr),
            (&out_t, &tgt_attr),
            ("sum_r", "real"),
            ("sum_i", "imag"),
        ]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bell_state() {
        // Create a 2-qubit circuit
        let mut qc = QuantumCircuit::new(2).unwrap();

        // Apply Hadamard to q0
        qc.h(0).unwrap();

        // Apply CNOT with q0 as control and q1 as target
        qc.cx(0, 1).unwrap();

        // The state should be the Bell state (|00> + |11>) / sqrt(2)
        // Verify state relation
        assert_eq!(qc.state.cardinality(), 2);

        for tuple in qc.state.tuples() {
            let q0 = tuple.get_typed::<i64>("q0").unwrap();
            let q1 = tuple.get_typed::<i64>("q1").unwrap();
            let real = tuple.get_typed::<f64>("real").unwrap();
            let imag = tuple.get_typed::<f64>("imag").unwrap();

            assert_eq!(imag, 0.0);
            // In the Bell state, q0 == q1
            assert_eq!(q0, q1);

            // The amplitude should be 1/sqrt(2)
            let s = std::f64::consts::FRAC_1_SQRT_2;
            assert!((real - s).abs() < 1e-6);
        }
    }

    #[test]
    fn test_x_gate() {
        let mut qc = QuantumCircuit::new(1).unwrap();
        qc.x(0).unwrap();

        assert_eq!(qc.state.cardinality(), 1);
        let tuple = qc.state.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("q0").unwrap(), 1);
        assert_eq!(tuple.get_typed::<f64>("real").unwrap(), 1.0);
    }
}
