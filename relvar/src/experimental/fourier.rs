//! Relational Discrete Fourier Transform (DFT).
//!
//! This module implements a discrete fourier transform purely using relational algebra.

use relvar_core::DatabaseError;
use relvar_core::algebra::Aggregation;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue};

/// A relational Discrete Fourier Transform.
pub struct RelationalDFT {
    /// Relation containing `(t: Int, amplitude: Float)`
    pub signal: Relation,
    /// Relation containing `(k: Int)`
    pub frequencies: Relation,
}

impl RelationalDFT {
    /// Creates a new RelationalDFT.
    pub fn new(signal: Relation, frequencies: Relation) -> Self {
        Self {
            signal,
            frequencies,
        }
    }

    /// Computes the Discrete Fourier Transform.
    ///
    /// # Returns
    ///
    /// A relation with heading `(k: Int, real: Float, imag: Float, magnitude: Float)`.
    pub fn compute(&self) -> Result<Relation, DatabaseError> {
        let n_val = self.signal.cardinality() as f64;

        // 1. Cross join signal and frequencies
        let cross = self.signal.join(&self.frequencies)?;

        // 2. Extend with real and imaginary parts
        let terms = cross
            .extend("real_term", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("t").unwrap() as f64;
                let k = t.get_typed::<i64>("k").unwrap() as f64;
                let amp = t.get_typed::<f64>("amplitude").unwrap();
                let theta = 2.0 * std::f64::consts::PI * k * n / n_val;
                ScalarValue::Float(amp * theta.cos())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("imag_term", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("t").unwrap() as f64;
                let k = t.get_typed::<i64>("k").unwrap() as f64;
                let amp = t.get_typed::<f64>("amplitude").unwrap();
                let theta = 2.0 * std::f64::consts::PI * k * n / n_val;
                ScalarValue::Float(-amp * theta.sin())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by frequency 'k'
        let spectrum = terms
            .summarize(
                &["k"],
                &[
                    Aggregation::sum_float("real", "real_term"),
                    Aggregation::sum_float("imag", "imag_term"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Extend with magnitude
        let result = spectrum
            .extend("magnitude", ScalarType::Float, |t| {
                let re = t.get_typed::<f64>("real").unwrap();
                let im = t.get_typed::<f64>("imag").unwrap();
                ScalarValue::Float((re * re + im * im).sqrt())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_dft_dc_signal() {
        let signal_heading = TupleType::new()
            .with_attribute("t", ScalarType::Int)
            .with_attribute("amplitude", ScalarType::Float);
        let mut signal = Relation::new(RelationType::new(signal_heading));

        // DC signal of amplitude 5 over 4 samples
        signal.insert(tuple! { t: 0i64, amplitude: 5.0 }).unwrap();
        signal.insert(tuple! { t: 1i64, amplitude: 5.0 }).unwrap();
        signal.insert(tuple! { t: 2i64, amplitude: 5.0 }).unwrap();
        signal.insert(tuple! { t: 3i64, amplitude: 5.0 }).unwrap();

        let freq_heading = TupleType::new().with_attribute("k", ScalarType::Int);
        let mut frequencies = Relation::new(RelationType::new(freq_heading));
        frequencies.insert(tuple! { k: 0i64 }).unwrap();
        frequencies.insert(tuple! { k: 1i64 }).unwrap();

        let dft = RelationalDFT::new(signal, frequencies);
        let spectrum = dft.compute().unwrap();

        assert_eq!(spectrum.cardinality(), 2);

        let k0 = spectrum
            .tuples()
            .find(|t| t.get_typed::<i64>("k") == Some(0))
            .unwrap();
        // k=0 is DC component, should be 4 * 5.0 = 20.0
        assert!((k0.get_typed::<f64>("real").unwrap() - 20.0).abs() < 1e-6);
        assert!((k0.get_typed::<f64>("imag").unwrap() - 0.0).abs() < 1e-6);
        assert!((k0.get_typed::<f64>("magnitude").unwrap() - 20.0).abs() < 1e-6);

        let k1 = spectrum
            .tuples()
            .find(|t| t.get_typed::<i64>("k") == Some(1))
            .unwrap();
        // k=1 should be 0 for DC signal
        assert!((k1.get_typed::<f64>("magnitude").unwrap() - 0.0).abs() < 1e-6);
    }
}
