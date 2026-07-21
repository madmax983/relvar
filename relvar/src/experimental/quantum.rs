//! Experimental Relational Quantum Circuit Simulator.
//!
//! This module models quantum state vectors and quantum gates purely using relational algebra.
//! A quantum state is represented as a relation mapping binary qubit values to complex amplitudes.
//! Applying a gate translates into relational joins (tensor products) and summarizations (matrix multiplications).

use relvar_core::DatabaseError;
use relvar_core::algebra::Aggregation;
use relvar_core::values::{ScalarValue, Tuple};
use relvar_core::{Relation, RelationType, ScalarType, TupleType, tuple};
use std::collections::BTreeMap;

/// Represents the state of an n-qubit quantum system.
#[derive(Clone, Debug)]
pub struct QuantumState {
    /// Number of qubits in the system.
    pub num_qubits: usize,
    /// The relational representation of the state amplitudes.
    pub state: Relation,
}

impl QuantumState {
    /// Creates a new quantum state initialized to |0...0>.
    pub fn new(num_qubits: usize) -> Result<Self, DatabaseError> {
        let mut heading = TupleType::new()
            .with_attribute("real", ScalarType::Float)
            .with_attribute("imag", ScalarType::Float);

        for i in 0..num_qubits {
            heading = heading.with_attribute(format!("q{}", i), ScalarType::Int);
        }

        let mut state = Relation::new(RelationType::new(heading.clone()));

        let total_states = 1 << num_qubits;
        for i in 0..total_states {
            let mut map = BTreeMap::new();
            if i == 0 {
                map.insert("real".to_string(), ScalarValue::Float(1.0));
                map.insert("imag".to_string(), ScalarValue::Float(0.0));
            } else {
                map.insert("real".to_string(), ScalarValue::Float(0.0));
                map.insert("imag".to_string(), ScalarValue::Float(0.0));
            }

            for q in 0..num_qubits {
                let bit = (i >> q) & 1;
                map.insert(format!("q{}", q), ScalarValue::Int(bit as i64));
            }

            let _ = state.insert(
                Tuple::new(heading.clone(), map)
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?,
            )?;
        }

        Ok(Self { num_qubits, state })
    }

    /// Applies a 1-qubit gate to the target qubit.
    pub fn apply_1q_gate(
        &self,
        target_qubit: usize,
        gate: &Relation,
    ) -> Result<Self, DatabaseError> {
        let q_attr = format!("q{}", target_qubit);

        let state_renamed = self.state.rename(&[(q_attr.as_str(), "in_bit")]);
        let joined = state_renamed.join(gate)?;

        let computed = joined
            .extend("new_real", ScalarType::Float, |t| {
                let r = t.get_typed::<f64>("real").unwrap();
                let i = t.get_typed::<f64>("imag").unwrap();
                let gr = t.get_typed::<f64>("gate_real").unwrap();
                let gi = t.get_typed::<f64>("gate_imag").unwrap();
                ScalarValue::Float(r * gr - i * gi)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_imag", ScalarType::Float, |t| {
                let r = t.get_typed::<f64>("real").unwrap();
                let i = t.get_typed::<f64>("imag").unwrap();
                let gr = t.get_typed::<f64>("gate_real").unwrap();
                let gi = t.get_typed::<f64>("gate_imag").unwrap();
                ScalarValue::Float(r * gi + i * gr)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let mut group_attrs = Vec::new();
        group_attrs.push("out_bit".to_string());
        for i in 0..self.num_qubits {
            if i != target_qubit {
                group_attrs.push(format!("q{}", i));
            }
        }

        let group_attrs_str: Vec<&str> = group_attrs.iter().map(|s| s.as_str()).collect();

        let summarized = computed
            .summarize(
                &group_attrs_str,
                &[
                    Aggregation::sum_float("sum_real", "new_real"),
                    Aggregation::sum_float("sum_imag", "new_imag"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let final_state = summarized.rename(&[
            ("sum_real", "real"),
            ("sum_imag", "imag"),
            ("out_bit", &q_attr),
        ]);

        Ok(Self {
            num_qubits: self.num_qubits,
            state: final_state,
        })
    }
}

/// Creates a Hadamard gate as a relation.
pub fn hadamard_gate() -> Relation {
    let heading = TupleType::new()
        .with_attribute("in_bit", ScalarType::Int)
        .with_attribute("out_bit", ScalarType::Int)
        .with_attribute("gate_real", ScalarType::Float)
        .with_attribute("gate_imag", ScalarType::Float);

    let mut gate = Relation::new(RelationType::new(heading));
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();

    // H|0> = 1/sqrt(2)|0> + 1/sqrt(2)|1>
    let _ = gate
        .insert(tuple! { in_bit: 0i64, out_bit: 0i64, gate_real: inv_sqrt2, gate_imag: 0.0 })
        .unwrap();
    let _ = gate
        .insert(tuple! { in_bit: 0i64, out_bit: 1i64, gate_real: inv_sqrt2, gate_imag: 0.0 })
        .unwrap();
    // H|1> = 1/sqrt(2)|0> - 1/sqrt(2)|1>
    let _ = gate
        .insert(tuple! { in_bit: 1i64, out_bit: 0i64, gate_real: inv_sqrt2, gate_imag: 0.0 })
        .unwrap();
    let _ = gate
        .insert(tuple! { in_bit: 1i64, out_bit: 1i64, gate_real: -inv_sqrt2, gate_imag: 0.0 })
        .unwrap();

    gate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_superposition() {
        // Create 1-qubit system
        let mut qs = QuantumState::new(1).unwrap();

        // Initial state is |0>
        let restricted = qs
            .state
            .restrict(|t| t.get_typed::<i64>("q0").unwrap() == 0);
        let t0 = restricted.tuples().next().unwrap();
        assert_eq!(t0.get_typed::<f64>("real").unwrap(), 1.0);

        // Apply Hadamard
        let h = hadamard_gate();
        qs = qs.apply_1q_gate(0, &h).unwrap();

        // Check state: should be 1/sqrt(2) for both |0> and |1>
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        for t in qs.state.tuples() {
            let r = t.get_typed::<f64>("real").unwrap();
            let diff = (r - inv_sqrt2).abs();
            assert!(diff < 1e-6);
        }
    }
}
