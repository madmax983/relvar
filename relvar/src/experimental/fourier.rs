//! Relational Discrete Fourier Transform (DFT)
//!
//! This module demonstrates how the Discrete Fourier Transform (DFT) can be
//! evaluated purely using relational algebra. It models the time-domain signal
//! as a relation and computes the frequency-domain spectrum using Cartesian
//! Products (Cross Joins), Extensions (Trigonometric functions), and Aggregations (Sum).
//!
//! # Concept
//!
//! - **Signal**: Relation `(n: Int, real: Float, imag: Float)` representing time-domain samples.
//! - **Frequencies**: Relation `(k: Int)` representing frequency bins.
//!
//! The DFT formula:
//! `X_k = sum_{n=0}^{N-1} x_n * e^{-i * 2 * pi * k * n / N}`
//!
//! Relational evaluation:
//! 1. Cross join `Signal` and `Frequencies` to get all `(n, k)` pairs.
//! 2. `Extend` to compute the complex multiplication terms.
//! 3. `Summarize` grouping by `k` and summing the real and imaginary parts.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};
use std::f64::consts::PI;

/// Relational Discrete Fourier Transform (DFT).
pub struct RelationalDft {
    /// Time-domain signal. Schema: `(n: Int, real: Float, imag: Float)`
    pub signal: Relation,
    /// Number of samples (N).
    pub n_samples: i64,
}

impl RelationalDft {
    /// Creates a new Relational DFT processor from a time-domain signal.
    pub fn new(signal: Relation, n_samples: i64) -> Self {
        Self { signal, n_samples }
    }

    /// Helper to create a frequency bins relation `(k: Int)` from 0 to N-1.
    fn create_frequencies(n_samples: i64) -> Result<Relation, DatabaseError> {
        let heading = TupleType::new().with_attribute("k", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading.clone()));
        for k in 0..n_samples {
            let mut vals = std::collections::BTreeMap::new();
            vals.insert("k".to_string(), ScalarValue::Int(k));
            rel.insert(Tuple::new(heading.clone(), vals).unwrap())?;
        }
        Ok(rel)
    }

    /// Computes the Discrete Fourier Transform.
    ///
    /// Returns the frequency-domain relation with schema `(k: Int, out_real: Float, out_imag: Float)`.
    pub fn compute(&self) -> Result<Relation, DatabaseError> {
        let freqs = Self::create_frequencies(self.n_samples)?;

        // 1. Cross join signal with frequencies (schemas are disjoint)
        let pairs = self.signal.join(&freqs)?;

        // 2 & 3. Compute real and imaginary parts of the term for each (n, k)
        let n_samples_f64 = self.n_samples as f64;

        let terms = pairs
            .extend("term_real", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("n").unwrap() as f64;
                let k = t.get_typed::<i64>("k").unwrap() as f64;
                let real = t.get_typed::<f64>("real").unwrap();
                let imag = t.get_typed::<f64>("imag").unwrap();

                let theta = -2.0 * PI * k * n / n_samples_f64;
                let term_real = real * theta.cos() - imag * theta.sin();

                ScalarValue::Float(term_real)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("term_imag", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("n").unwrap() as f64;
                let k = t.get_typed::<i64>("k").unwrap() as f64;
                let real = t.get_typed::<f64>("real").unwrap();
                let imag = t.get_typed::<f64>("imag").unwrap();

                let theta = -2.0 * PI * k * n / n_samples_f64;
                let term_imag = real * theta.sin() + imag * theta.cos();

                ScalarValue::Float(term_imag)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize by k, summing term_real and term_imag
        let spectrum = terms
            .summarize(
                &["k"],
                &[
                    Aggregation::sum_float("out_real", "term_real"),
                    Aggregation::sum_float("out_imag", "term_imag"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(spectrum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_dft_dc_signal() {
        // A DC signal of amplitude 1.0 has a DFT of N at k=0, and 0 elsewhere.
        let n_samples = 4;
        let heading = TupleType::new()
            .with_attribute("n", ScalarType::Int)
            .with_attribute("real", ScalarType::Float)
            .with_attribute("imag", ScalarType::Float);

        let mut signal = Relation::new(RelationType::new(heading));
        for n in 0..n_samples {
            signal
                .insert(tuple! { n: n, real: 1.0f64, imag: 0.0f64 })
                .unwrap();
        }

        let dft = RelationalDft::new(signal, n_samples);
        let spectrum = dft.compute().unwrap();

        assert_eq!(spectrum.cardinality(), 4);

        for t in spectrum.tuples() {
            let k = t.get_typed::<i64>("k").unwrap();
            let out_real = t.get_typed::<f64>("out_real").unwrap();
            let out_imag = t.get_typed::<f64>("out_imag").unwrap();

            if k == 0 {
                assert!((out_real - 4.0).abs() < 1e-6);
                assert!(out_imag.abs() < 1e-6);
            } else {
                assert!(out_real.abs() < 1e-6);
                assert!(out_imag.abs() < 1e-6);
            }
        }
    }
}
