//! Relational Quantum Circuit Simulator.
//!
//! This module demonstrates how quantum states and quantum logic gates can be modeled
//! purely using relational algebra.
//!
//! # Concept
//!
//! - **Quantum State**: A column vector of complex amplitudes, modeled as a relation
//!   with heading `(index: Int, real: Float, imag: Float)`.
//! - **Quantum Gate**: A complex matrix, modeled as a relation with heading
//!   `(row: Int, col: Int, real: Float, imag: Float)`.
//! - **Gate Application**: Applying a gate to a state is simply complex matrix
//!   multiplication. This is implemented via a Relational Join on `col = index`,
//!   an Extension to compute complex multiplication, and a Summarization to sum
//!   the amplitudes.
//! - **Tensor Product**: Combining gates (e.g., $H \otimes I$) is implemented using
//!   a Relational Cross Join followed by Extensions to calculate the new coordinates
//!   and complex product.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A quantum state vector.
pub struct QuantumState {
    /// The relation storing the complex amplitudes.
    /// Heading: `(index: Int, real: Float, imag: Float)`
    pub relation: Relation,
}

impl QuantumState {
    /// Creates a new, empty quantum state.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("index", ScalarType::Int)
            .with_attribute("real", ScalarType::Float)
            .with_attribute("imag", ScalarType::Float);

        Self {
            relation: Relation::new(RelationType::new(heading)),
        }
    }

    /// Initializes a pure state $|0\dots0\rangle$.
    pub fn zero_state() -> Result<Self, DatabaseError> {
        let mut state = Self::new();
        state
            .relation
            .insert(tuple! { index: 0, real: 1.0, imag: 0.0 })
            .unwrap();
        Ok(state)
    }

    /// Applies a quantum gate to this state.
    pub fn apply(&self, gate: &QuantumGate) -> Result<QuantumState, DatabaseError> {
        // State: (index, s_real, s_imag) -> rename to (col, s_real, s_imag)
        let state_renamed =
            self.relation
                .rename(&[("index", "col"), ("real", "s_real"), ("imag", "s_imag")]);

        // Join Gate and State on `col`
        let joined = gate
            .relation
            .join(&state_renamed)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Extend with complex multiplication: (real + i*imag) * (s_real + i*s_imag)
        // new_real = real * s_real - imag * s_imag
        // new_imag = real * s_imag + imag * s_real
        let extended = joined
            .extend("new_real", ScalarType::Float, |t| {
                let r = t.get_typed::<f64>("real").unwrap();
                let i = t.get_typed::<f64>("imag").unwrap();
                let sr = t.get_typed::<f64>("s_real").unwrap();
                let si = t.get_typed::<f64>("s_imag").unwrap();
                ScalarValue::Float(r * sr - i * si)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_imag", ScalarType::Float, |t| {
                let r = t.get_typed::<f64>("real").unwrap();
                let i = t.get_typed::<f64>("imag").unwrap();
                let sr = t.get_typed::<f64>("s_real").unwrap();
                let si = t.get_typed::<f64>("s_imag").unwrap();
                ScalarValue::Float(r * si + i * sr)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize by `row`, summing the new real and imaginary parts
        let summarized = extended
            .summarize(
                &["row"],
                &[
                    Aggregation::sum_float("sum_real", "new_real"),
                    Aggregation::sum_float("sum_imag", "new_imag"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename back to state heading
        let final_relation =
            summarized.rename(&[("row", "index"), ("sum_real", "real"), ("sum_imag", "imag")]);

        Ok(Self {
            relation: final_relation,
        })
    }
}

/// A quantum logic gate (complex matrix).
pub struct QuantumGate {
    /// The relation storing the matrix entries.
    /// Heading: `(row: Int, col: Int, real: Float, imag: Float)`
    pub relation: Relation,
}

impl QuantumGate {
    /// Creates a new, empty quantum gate.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("row", ScalarType::Int)
            .with_attribute("col", ScalarType::Int)
            .with_attribute("real", ScalarType::Float)
            .with_attribute("imag", ScalarType::Float);

        Self {
            relation: Relation::new(RelationType::new(heading)),
        }
    }

    /// Creates a Hadamard gate (H).
    pub fn hadamard() -> Self {
        let mut gate = Self::new();
        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
        gate.relation
            .insert(tuple! { row: 0, col: 0, real: inv_sqrt2, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 0, col: 1, real: inv_sqrt2, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 1, col: 0, real: inv_sqrt2, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 1, col: 1, real: -inv_sqrt2, imag: 0.0 })
            .unwrap();
        gate
    }

    /// Creates a Pauli-X gate (NOT).
    pub fn pauli_x() -> Self {
        let mut gate = Self::new();
        gate.relation
            .insert(tuple! { row: 0, col: 1, real: 1.0, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 1, col: 0, real: 1.0, imag: 0.0 })
            .unwrap();
        gate
    }

    /// Creates a Pauli-Z gate.
    pub fn pauli_z() -> Self {
        let mut gate = Self::new();
        gate.relation
            .insert(tuple! { row: 0, col: 0, real: 1.0, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 1, col: 1, real: -1.0, imag: 0.0 })
            .unwrap();
        gate
    }

    /// Creates a Controlled-NOT (CNOT) gate.
    pub fn cnot() -> Self {
        let mut gate = Self::new();
        gate.relation
            .insert(tuple! { row: 0, col: 0, real: 1.0, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 1, col: 1, real: 1.0, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 2, col: 3, real: 1.0, imag: 0.0 })
            .unwrap();
        gate.relation
            .insert(tuple! { row: 3, col: 2, real: 1.0, imag: 0.0 })
            .unwrap();
        gate
    }

    /// Computes the Kronecker product of two gates.
    pub fn tensor_product(
        &self,
        other: &QuantumGate,
        size2: i64,
    ) -> Result<QuantumGate, DatabaseError> {
        let a = self.relation.rename(&[
            ("row", "r1"),
            ("col", "c1"),
            ("real", "real1"),
            ("imag", "imag1"),
        ]);
        let b = other.relation.rename(&[
            ("row", "r2"),
            ("col", "c2"),
            ("real", "real2"),
            ("imag", "imag2"),
        ]);

        // Cross join
        let joined = a
            .join(&b)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Calculate new coordinates and values
        let extended = joined
            .extend("row", ScalarType::Int, move |t| {
                let r1 = t.get_typed::<i64>("r1").unwrap();
                let r2 = t.get_typed::<i64>("r2").unwrap();
                ScalarValue::Int(r1 * size2 + r2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("col", ScalarType::Int, move |t| {
                let c1 = t.get_typed::<i64>("c1").unwrap();
                let c2 = t.get_typed::<i64>("c2").unwrap();
                ScalarValue::Int(c1 * size2 + c2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("real", ScalarType::Float, |t| {
                let r1 = t.get_typed::<f64>("real1").unwrap();
                let i1 = t.get_typed::<f64>("imag1").unwrap();
                let r2 = t.get_typed::<f64>("real2").unwrap();
                let i2 = t.get_typed::<f64>("imag2").unwrap();
                ScalarValue::Float(r1 * r2 - i1 * i2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("imag", ScalarType::Float, |t| {
                let r1 = t.get_typed::<f64>("real1").unwrap();
                let i1 = t.get_typed::<f64>("imag1").unwrap();
                let r2 = t.get_typed::<f64>("real2").unwrap();
                let i2 = t.get_typed::<f64>("imag2").unwrap();
                ScalarValue::Float(r1 * i2 + i1 * r2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let projected = extended.project(&["row", "col", "real", "imag"]);

        Ok(QuantumGate {
            relation: projected,
        })
    }
}

impl Default for QuantumState {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for QuantumGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_amp(state: &QuantumState, index: i64) -> (f64, f64) {
        for t in state.relation.tuples() {
            if t.get_typed::<i64>("index") == Some(index) {
                return (
                    t.get_typed::<f64>("real").unwrap(),
                    t.get_typed::<f64>("imag").unwrap(),
                );
            }
        }
        (0.0, 0.0)
    }

    #[test]
    fn test_pauli_x() {
        let state = QuantumState::zero_state().unwrap();
        let gate = QuantumGate::pauli_x();
        let result = state.apply(&gate).unwrap();

        // |0> -> |1>
        assert_eq!(get_amp(&result, 0).0, 0.0);
        assert_eq!(get_amp(&result, 1).0, 1.0);
    }

    #[test]
    fn test_hadamard() {
        let state = QuantumState::zero_state().unwrap();
        let gate = QuantumGate::hadamard();
        let result = state.apply(&gate).unwrap();

        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
        // Should be in superposition |0> + |1>
        assert!((get_amp(&result, 0).0 - inv_sqrt2).abs() < 1e-6);
        assert!((get_amp(&result, 1).0 - inv_sqrt2).abs() < 1e-6);
    }

    #[test]
    fn test_bell_state() {
        // Create |00> state
        let mut state = QuantumState::new();
        state
            .relation
            .insert(tuple! { index: 0, real: 1.0, imag: 0.0 })
            .unwrap();

        // Apply H to first qubit (H tensor I)
        let h = QuantumGate::hadamard();
        let mut i = QuantumGate::new();
        i.relation
            .insert(tuple! { row: 0, col: 0, real: 1.0, imag: 0.0 })
            .unwrap();
        i.relation
            .insert(tuple! { row: 1, col: 1, real: 1.0, imag: 0.0 })
            .unwrap();

        let h_i = h.tensor_product(&i, 2).unwrap();
        let step1 = state.apply(&h_i).unwrap();

        // Apply CNOT
        let cnot = QuantumGate::cnot();
        let final_state = step1.apply(&cnot).unwrap();

        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
        // Bell state |Phi+> = (|00> + |11>) / sqrt(2)
        assert!((get_amp(&final_state, 0).0 - inv_sqrt2).abs() < 1e-6);
        assert_eq!(get_amp(&final_state, 1).0, 0.0);
        assert_eq!(get_amp(&final_state, 2).0, 0.0);
        assert!((get_amp(&final_state, 3).0 - inv_sqrt2).abs() < 1e-6);
    }
}
