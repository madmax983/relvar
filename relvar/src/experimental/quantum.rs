//! Relational Quantum Circuit Simulator
//!
//! This module models a quantum computer simulator using purely relational algebra.
//! - The **quantum state** is a relation: `(basis: Int, amplitude_real: Float, amplitude_imag: Float)`.
//!   It acts as a sparse state vector.
//! - A **quantum gate** (like Hadamard or Pauli-X) is applied via `Join` with a gate transition
//!   relation, followed by `Summarize` to compute quantum interference (adding complex amplitudes).

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// A Relational Quantum Simulator
///
/// # Examples
///
/// ```
/// use relvar_core::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::quantum::QuantumSimulator;
///
/// let mut sim = QuantumSimulator::new();
/// // Qubit 0 in superposition
/// sim.apply_hadamard(0).unwrap();
/// ```
pub struct QuantumSimulator {
    /// The sparse quantum state.
    /// Schema: (basis: Int, amplitude_real: Float, amplitude_imag: Float)
    pub state: Relation,
}

impl QuantumSimulator {
    /// Creates a new Quantum Simulator with the state |0...0>
    pub fn new() -> Self {
        let state_type = RelationType::new(
            TupleType::new()
                .with_attribute("basis".to_string(), ScalarType::Int)
                .with_attribute("amplitude_real".to_string(), ScalarType::Float)
                .with_attribute("amplitude_imag".to_string(), ScalarType::Float),
        );
        let mut state = Relation::new(state_type);
        // Initial state |0> with amplitude 1 + 0i
        state
            .insert(tuple! { basis: 0i64, amplitude_real: 1.0f64, amplitude_imag: 0.0f64 })
            .unwrap();

        Self { state }
    }

    /// Applies a generic 1-qubit gate using a relation matrix
    pub fn apply_1q_gate(
        &mut self,
        target_qubit: i64,
        gate_matrix: &Relation,
    ) -> Result<(), DatabaseError> {
        // gate_matrix schema: (in_bit: Int, out_bit: Int, real: Float, imag: Float)

        // 1. Extract the target bit and the remaining bits from the basis
        let state_extended = self
            .state
            .extend("in_bit", ScalarType::Int, move |t| {
                let basis = t.get_typed::<i64>("basis").unwrap();
                ScalarValue::Int((basis >> target_qubit) & 1)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("rest", ScalarType::Int, move |t| {
                let basis = t.get_typed::<i64>("basis").unwrap();
                let in_bit = (basis >> target_qubit) & 1;
                ScalarValue::Int(basis - (in_bit << target_qubit))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Join the state with the gate matrix (matches on in_bit)
        let joined = state_extended.join(gate_matrix)?;

        // 3. Compute the new amplitudes using complex multiplication and form the new basis
        let computed = joined
            .extend("new_basis", ScalarType::Int, move |t| {
                let rest = t.get_typed::<i64>("rest").unwrap();
                let out_bit = t.get_typed::<i64>("out_bit").unwrap();
                ScalarValue::Int(rest | (out_bit << target_qubit))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_real", ScalarType::Float, |t| {
                let a_real = t.get_typed::<f64>("amplitude_real").unwrap();
                let a_imag = t.get_typed::<f64>("amplitude_imag").unwrap();
                let g_real = t.get_typed::<f64>("real").unwrap();
                let g_imag = t.get_typed::<f64>("imag").unwrap();
                ScalarValue::Float(a_real * g_real - a_imag * g_imag)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_imag", ScalarType::Float, |t| {
                let a_real = t.get_typed::<f64>("amplitude_real").unwrap();
                let a_imag = t.get_typed::<f64>("amplitude_imag").unwrap();
                let g_real = t.get_typed::<f64>("real").unwrap();
                let g_imag = t.get_typed::<f64>("imag").unwrap();
                ScalarValue::Float(a_real * g_imag + a_imag * g_real)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize by the new basis to combine overlapping amplitudes (Quantum Interference)
        let next_state_raw = computed
            .summarize(
                &["new_basis"],
                &[
                    Aggregation::sum_float("amplitude_real_sum", "new_real"),
                    Aggregation::sum_float("amplitude_imag_sum", "new_imag"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Rename attributes to standard state schema
        self.state = next_state_raw.rename(&[
            ("new_basis", "basis"),
            ("amplitude_real_sum", "amplitude_real"),
            ("amplitude_imag_sum", "amplitude_imag"),
        ]);

        // 6. Filter out near-zero amplitudes to maintain sparse representation
        self.state = self.state.restrict(|t| {
            let r = t.get_typed::<f64>("amplitude_real").unwrap();
            let i = t.get_typed::<f64>("amplitude_imag").unwrap();
            r.abs() > 1e-10 || i.abs() > 1e-10
        });

        Ok(())
    }

    /// Applies a Hadamard (H) gate to create a superposition.
    pub fn apply_hadamard(&mut self, target_qubit: i64) -> Result<(), DatabaseError> {
        let gate_type = RelationType::new(
            TupleType::new()
                .with_attribute("in_bit".to_string(), ScalarType::Int)
                .with_attribute("out_bit".to_string(), ScalarType::Int)
                .with_attribute("real".to_string(), ScalarType::Float)
                .with_attribute("imag".to_string(), ScalarType::Float),
        );
        let mut h_gate = Relation::new(gate_type);
        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;

        h_gate
            .insert(tuple! { in_bit: 0i64, out_bit: 0i64, real: inv_sqrt2, imag: 0.0f64 })
            .unwrap();
        h_gate
            .insert(tuple! { in_bit: 0i64, out_bit: 1i64, real: inv_sqrt2, imag: 0.0f64 })
            .unwrap();
        h_gate
            .insert(tuple! { in_bit: 1i64, out_bit: 0i64, real: inv_sqrt2, imag: 0.0f64 })
            .unwrap();
        h_gate
            .insert(tuple! { in_bit: 1i64, out_bit: 1i64, real: -inv_sqrt2, imag: 0.0f64 })
            .unwrap();

        self.apply_1q_gate(target_qubit, &h_gate)
    }

    /// Applies a Pauli-X (NOT) gate to flip the qubit.
    pub fn apply_x(&mut self, target_qubit: i64) -> Result<(), DatabaseError> {
        let gate_type = RelationType::new(
            TupleType::new()
                .with_attribute("in_bit".to_string(), ScalarType::Int)
                .with_attribute("out_bit".to_string(), ScalarType::Int)
                .with_attribute("real".to_string(), ScalarType::Float)
                .with_attribute("imag".to_string(), ScalarType::Float),
        );
        let mut x_gate = Relation::new(gate_type);

        x_gate
            .insert(tuple! { in_bit: 0i64, out_bit: 1i64, real: 1.0f64, imag: 0.0f64 })
            .unwrap();
        x_gate
            .insert(tuple! { in_bit: 1i64, out_bit: 0i64, real: 1.0f64, imag: 0.0f64 })
            .unwrap();

        self.apply_1q_gate(target_qubit, &x_gate)
    }

    /// Applies a Controlled-NOT (CNOT) gate.
    /// Uses pure relational extensions to flip the target qubit conditionally.
    pub fn apply_cnot(
        &mut self,
        control_qubit: i64,
        target_qubit: i64,
    ) -> Result<(), DatabaseError> {
        let extended = self
            .state
            .extend("new_basis", ScalarType::Int, move |t| {
                let basis = t.get_typed::<i64>("basis").unwrap();
                let control_bit = (basis >> control_qubit) & 1;
                if control_bit == 1 {
                    ScalarValue::Int(basis ^ (1 << target_qubit))
                } else {
                    ScalarValue::Int(basis)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let renamed = extended
            .project(&["new_basis", "amplitude_real", "amplitude_imag"])
            .rename(&[("new_basis", "basis")]);

        self.state = renamed;
        Ok(())
    }
}

impl Default for QuantumSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_superposition() {
        let mut sim = QuantumSimulator::new();
        // Initially in |0>
        assert_eq!(sim.state.cardinality(), 1);

        // Apply H to qubit 0
        sim.apply_hadamard(0).unwrap();

        // State should be 1/sqrt(2) |0> + 1/sqrt(2) |1>
        assert_eq!(sim.state.cardinality(), 2);

        let mut amplitudes = std::collections::HashMap::new();
        for t in sim.state.tuples() {
            let basis = t.get_typed::<i64>("basis").unwrap();
            let real = t.get_typed::<f64>("amplitude_real").unwrap();
            amplitudes.insert(basis, real);
        }

        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
        assert!((amplitudes[&0] - inv_sqrt2).abs() < 1e-6);
        assert!((amplitudes[&1] - inv_sqrt2).abs() < 1e-6);
    }

    #[test]
    fn test_interference() {
        let mut sim = QuantumSimulator::new();

        // Apply H twice to qubit 0
        sim.apply_hadamard(0).unwrap();
        sim.apply_hadamard(0).unwrap();

        // State should perfectly interfere back to |0>
        assert_eq!(sim.state.cardinality(), 1);
        let t = sim.state.tuples().next().unwrap();
        assert_eq!(t.get_typed::<i64>("basis").unwrap(), 0);
        assert!((t.get_typed::<f64>("amplitude_real").unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bell_state_entanglement() {
        let mut sim = QuantumSimulator::new();

        // Create a Bell State: H on qubit 0, CNOT control 0 target 1
        sim.apply_hadamard(0).unwrap();
        sim.apply_cnot(0, 1).unwrap();

        // Expected state: 1/sqrt(2) (|00> + |11>)
        // Basis 0 (00) and Basis 3 (11)
        assert_eq!(sim.state.cardinality(), 2);

        let mut amplitudes = std::collections::HashMap::new();
        for t in sim.state.tuples() {
            let basis = t.get_typed::<i64>("basis").unwrap();
            let real = t.get_typed::<f64>("amplitude_real").unwrap();
            amplitudes.insert(basis, real);
        }

        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;
        assert!((amplitudes[&0] - inv_sqrt2).abs() < 1e-6);
        assert!((amplitudes[&3] - inv_sqrt2).abs() < 1e-6);
    }
}
