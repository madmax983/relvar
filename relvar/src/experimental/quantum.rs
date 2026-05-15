//! Relational Quantum Circuit Simulator
//!
//! A simulator for quantum circuits using purely relational algebra.
//! Quantum states are represented as relations of probability amplitudes,
//! and quantum gates are represented as relations mapping input states to output states.
//! Applying a gate is performed via Relational Joins and Aggregations (Matrix Multiplication).
//! Multi-qubit gates are generated dynamically using Relational Kronecker Products (Cross Joins).

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// A quantum state representing the probability amplitudes of the basis states.
pub struct QuantumState {
    /// Relation with schema `(state_id: Int, real: Float, imag: Float)`
    pub amplitudes: Relation,
    /// Number of qubits in this state
    pub num_qubits: usize,
}

impl QuantumState {
    /// Creates a new n-qubit quantum state initialized to |0...0>.
    pub fn new(num_qubits: usize) -> Self {
        let heading = TupleType::new()
            .with_attribute("state_id", ScalarType::Int)
            .with_attribute("real", ScalarType::Float)
            .with_attribute("imag", ScalarType::Float);

        let mut amplitudes = Relation::new(RelationType::new(heading));

        let num_states = 1 << num_qubits;
        for i in 0..num_states {
            let (real, imag) = if i == 0 { (1.0, 0.0) } else { (0.0, 0.0) };
            amplitudes
                .insert(tuple! {
                    state_id: i as i64,
                    real: real,
                    imag: imag
                })
                .unwrap();
        }

        Self {
            amplitudes,
            num_qubits,
        }
    }

    /// Applies a unitary gate matrix to the quantum state.
    /// The gate must be a relation with schema `(row_id: Int, col_id: Int, real: Float, imag: Float)`.
    pub fn apply_gate(&self, gate: &Relation) -> Result<QuantumState, DatabaseError> {
        let renamed_amps =
            self.amplitudes
                .rename(&[("state_id", "col_id"), ("real", "amp_r"), ("imag", "amp_i")]);

        let renamed_gate = gate.rename(&[("real", "gate_r"), ("imag", "gate_i")]);

        let joined = renamed_amps.join(&renamed_gate)?;

        let multiplied = joined
            .extend("new_r", ScalarType::Float, |t| {
                let ar = t.get_typed::<f64>("amp_r").unwrap();
                let ai = t.get_typed::<f64>("amp_i").unwrap();
                let gr = t.get_typed::<f64>("gate_r").unwrap();
                let gi = t.get_typed::<f64>("gate_i").unwrap();
                ScalarValue::Float(ar * gr - ai * gi)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_i", ScalarType::Float, |t| {
                let ar = t.get_typed::<f64>("amp_r").unwrap();
                let ai = t.get_typed::<f64>("amp_i").unwrap();
                let gr = t.get_typed::<f64>("gate_r").unwrap();
                let gi = t.get_typed::<f64>("gate_i").unwrap();
                ScalarValue::Float(ar * gi + ai * gr)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let new_state = multiplied
            .summarize(
                &["row_id"],
                &[
                    Aggregation::sum_float("real", "new_r"),
                    Aggregation::sum_float("imag", "new_i"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let final_amplitudes = new_state.rename(&[("row_id", "state_id")]);

        Ok(QuantumState {
            amplitudes: final_amplitudes,
            num_qubits: self.num_qubits,
        })
    }
}

/// Helper function to create a 2x2 identity gate
pub fn identity_gate() -> Relation {
    let heading = TupleType::new()
        .with_attribute("row_id", ScalarType::Int)
        .with_attribute("col_id", ScalarType::Int)
        .with_attribute("real", ScalarType::Float)
        .with_attribute("imag", ScalarType::Float);

    let mut gate = Relation::new(RelationType::new(heading));
    gate.insert(tuple! { row_id: 0i64, col_id: 0i64, real: 1.0f64, imag: 0.0f64 })
        .unwrap();
    gate.insert(tuple! { row_id: 0i64, col_id: 1i64, real: 0.0f64, imag: 0.0f64 })
        .unwrap();
    gate.insert(tuple! { row_id: 1i64, col_id: 0i64, real: 0.0f64, imag: 0.0f64 })
        .unwrap();
    gate.insert(tuple! { row_id: 1i64, col_id: 1i64, real: 1.0f64, imag: 0.0f64 })
        .unwrap();
    gate
}

/// Helper function to create a 2x2 Hadamard gate
pub fn hadamard_gate() -> Relation {
    let heading = TupleType::new()
        .with_attribute("row_id", ScalarType::Int)
        .with_attribute("col_id", ScalarType::Int)
        .with_attribute("real", ScalarType::Float)
        .with_attribute("imag", ScalarType::Float);

    let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
    let mut gate = Relation::new(RelationType::new(heading));
    gate.insert(tuple! { row_id: 0i64, col_id: 0i64, real: inv_sqrt2, imag: 0.0f64 })
        .unwrap();
    gate.insert(tuple! { row_id: 0i64, col_id: 1i64, real: inv_sqrt2, imag: 0.0f64 })
        .unwrap();
    gate.insert(tuple! { row_id: 1i64, col_id: 0i64, real: inv_sqrt2, imag: 0.0f64 })
        .unwrap();
    gate.insert(tuple! { row_id: 1i64, col_id: 1i64, real: -inv_sqrt2, imag: 0.0f64 })
        .unwrap();
    gate
}

/// Computes the Kronecker product of two gate matrices using Relational Algebra.
pub fn kronecker_product(
    gate_a: &Relation,
    gate_b: &Relation,
    size_b: i64,
) -> Result<Relation, DatabaseError> {
    let a_renamed = gate_a.rename(&[
        ("row_id", "row_a"),
        ("col_id", "col_a"),
        ("real", "real_a"),
        ("imag", "imag_a"),
    ]);

    let b_renamed = gate_b.rename(&[
        ("row_id", "row_b"),
        ("col_id", "col_b"),
        ("real", "real_b"),
        ("imag", "imag_b"),
    ]);

    // Cross join
    let cross_joined = a_renamed.join(&b_renamed)?;

    let expanded = cross_joined
        .extend("row_id", ScalarType::Int, move |t| {
            let ra = t.get_typed::<i64>("row_a").unwrap();
            let rb = t.get_typed::<i64>("row_b").unwrap();
            ScalarValue::Int(ra * size_b + rb)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        .extend("col_id", ScalarType::Int, move |t| {
            let ca = t.get_typed::<i64>("col_a").unwrap();
            let cb = t.get_typed::<i64>("col_b").unwrap();
            ScalarValue::Int(ca * size_b + cb)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        .extend("real", ScalarType::Float, |t| {
            let ra = t.get_typed::<f64>("real_a").unwrap();
            let ia = t.get_typed::<f64>("imag_a").unwrap();
            let rb = t.get_typed::<f64>("real_b").unwrap();
            let ib = t.get_typed::<f64>("imag_b").unwrap();
            ScalarValue::Float(ra * rb - ia * ib)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
        .extend("imag", ScalarType::Float, |t| {
            let ra = t.get_typed::<f64>("real_a").unwrap();
            let ia = t.get_typed::<f64>("imag_a").unwrap();
            let rb = t.get_typed::<f64>("real_b").unwrap();
            let ib = t.get_typed::<f64>("imag_b").unwrap();
            ScalarValue::Float(ra * ib + ia * rb)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let result = expanded.project(&["row_id", "col_id", "real", "imag"]);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_state_initialization() {
        let state = QuantumState::new(2); // 2 qubits => 4 states
        assert_eq!(state.amplitudes.cardinality(), 4);

        let t0 = state
            .amplitudes
            .tuples()
            .find(|t| t.get_typed::<i64>("state_id") == Some(0))
            .unwrap();
        assert_eq!(t0.get_typed::<f64>("real"), Some(1.0));

        let t1 = state
            .amplitudes
            .tuples()
            .find(|t| t.get_typed::<i64>("state_id") == Some(1))
            .unwrap();
        assert_eq!(t1.get_typed::<f64>("real"), Some(0.0));
    }

    #[test]
    fn test_kronecker_product() {
        let i = identity_gate();
        let h = hadamard_gate();
        let ih = kronecker_product(&i, &h, 2).unwrap();

        // I ⊗ H should be a 4x4 matrix, meaning 16 tuples
        assert_eq!(ih.cardinality(), 16);
    }

    #[test]
    fn test_apply_hadamard() {
        let state = QuantumState::new(1); // |0>
        let h = hadamard_gate();
        let new_state = state.apply_gate(&h).unwrap();

        // Output should be |+> = 1/sqrt(2) |0> + 1/sqrt(2) |1>
        let t0 = new_state
            .amplitudes
            .tuples()
            .find(|t| t.get_typed::<i64>("state_id") == Some(0))
            .unwrap();
        let t1 = new_state
            .amplitudes
            .tuples()
            .find(|t| t.get_typed::<i64>("state_id") == Some(1))
            .unwrap();

        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
        assert!((t0.get_typed::<f64>("real").unwrap() - inv_sqrt2).abs() < 1e-6);
        assert!((t1.get_typed::<f64>("real").unwrap() - inv_sqrt2).abs() < 1e-6);
    }
}
